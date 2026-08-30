// Local TTS engine module (formerly the sbv2-local-tts crate, now embedded).

pub mod adapter;
pub mod engine;
pub mod model_manager;
pub mod package;
pub mod paths;
pub mod registry;
pub mod setup;
pub mod sherpa_onnx_manager;

mod download;
mod saf_bridge;

use std::sync::Arc;

/// 懒加载的 HTTP 客户端，供 Sherpa-ONNX 多文件下载使用。
fn download_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        crate::utils::download::build_download_client().expect("build download client")
    })
}

/// 构建向 Tauri 前端发射 `tts://download-progress` 的进度回调（供 Sherpa 下载复用）。
type ProgressCb = std::sync::Arc<dyn Fn(crate::utils::download::DownloadProgress) + Send + Sync>;

fn sherpa_progress_cb(
    app: &AppHandle,
    asset_id: &str,
) -> ProgressCb {
    let app = app.clone();
    let asset_id = asset_id.to_string();
    std::sync::Arc::new(move |p| {
        let _ = app.emit(
            "tts://download-progress",
            download::DownloadProgress {
                asset_id: asset_id.clone(),
                bytes_done: p.bytes_done,
                total_bytes: p.total_bytes,
                percent: p.percent,
            },
        );
    })
}

pub use engine::{LocalTtsEngine, SynthesizeRequest};
pub use paths::LocalTtsPaths;

// ---------------------------------------------------------------------------
// LocalTtsState -- Tauri managed state for the local TTS engine
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

pub struct LocalTtsState {
    pub paths: LocalTtsPaths,
    pub engine: Arc<LocalTtsEngine>,
    pub cancel: tokio::sync::Mutex<Option<Arc<CancellationToken>>>,
}

impl LocalTtsState {
    pub fn new(paths: LocalTtsPaths) -> Self {
        Self {
            paths,
            engine: Arc::new(LocalTtsEngine::new()),
            cancel: tokio::sync::Mutex::new(None),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TtsLocalStatus {
    pub ready: bool,
    pub deberta_installed: bool,
    pub installed_voice_count: usize,
}

#[derive(Debug, Serialize)]
pub struct TtsLocalInstallSnapshot {
    pub assets: Vec<model_manager::AssetRecord>,
    pub voices: Vec<model_manager::VoiceRecord>,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub asset_id: String,
    pub voice_id: Option<String>,
    pub path: String,
    pub bytes: u64,
    pub message: String,
}

// ---------------------------------------------------------------------------
// LocalTtsSwitch -- runtime enable/disable gate
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};

use crate::config;

#[derive(Clone, Debug)]
pub struct LocalTtsSwitch {
    enabled: Arc<AtomicBool>,
}

/// 本地 TTS 进程内引擎的共享运行时依赖。三合一后作为单个参数
/// 从 lib.rs → init → AIService → RoleManager → VoiceMaker 传递，
/// 避免把 engine/paths/switch 三个散装字段逐层展开。
#[derive(Clone, Debug)]
pub struct LocalTtsRuntime {
    pub engine: Arc<LocalTtsEngine>,
    pub paths: LocalTtsPaths,
    pub switch: LocalTtsSwitch,
}

impl LocalTtsRuntime {
    pub fn new(
        engine: Arc<LocalTtsEngine>,
        paths: LocalTtsPaths,
        switch: LocalTtsSwitch,
    ) -> Self {
        Self {
            engine,
            paths,
            switch,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.switch.is_enabled()
    }
}

impl LocalTtsSwitch {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalTtsSwitchStatus {
    pub configured_enabled: bool,
    pub effective_enabled: bool,
}

fn read_configured_enabled(app: &AppHandle) -> Result<bool, String> {
    let store = config::settings_store(app).map_err(|e| e.to_string())?;
    Ok(store
        .get(config::keys::ENABLE_LOCAL_TTS)
        .and_then(|value| value.as_bool())
        .unwrap_or(false))
}

pub fn load_configured_enabled(app: &AppHandle) -> bool {
    read_configured_enabled(app).unwrap_or(false)
}

/// 读取持久化的推理设备配置（`features.local_tts_device`）。
/// 返回 `None` 表示未配置（用引擎默认 CPU）。
pub fn read_configured_device(app: &AppHandle) -> Option<sbv2_core::model::InferenceDevice> {
    crate::utils::device::read_configured_device(app, crate::config::keys::LOCAL_TTS_DEVICE)
}

// ---------------------------------------------------------------------------
// Tauri commands -- switch management
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn tts_local_get_enabled(
    app: AppHandle,
    switch: State<'_, LocalTtsSwitch>,
) -> Result<LocalTtsSwitchStatus, String> {
    Ok(LocalTtsSwitchStatus {
        configured_enabled: read_configured_enabled(&app)?,
        effective_enabled: switch.is_enabled(),
    })
}

#[tauri::command]
pub async fn tts_local_set_enabled(
    app: AppHandle,
    switch: State<'_, LocalTtsSwitch>,
    local_state: State<'_, LocalTtsState>,
    enabled: bool,
) -> Result<LocalTtsSwitchStatus, String> {
    if enabled {
        local_state.paths.ensure()?;
    }

    let store = config::settings_store(&app).map_err(|e| e.to_string())?;
    let previous = store.get(config::keys::ENABLE_LOCAL_TTS);
    store.set(config::keys::ENABLE_LOCAL_TTS, enabled);
    if let Err(error) = store.save() {
        if let Some(value) = previous {
            store.set(config::keys::ENABLE_LOCAL_TTS, value);
        } else {
            store.delete(config::keys::ENABLE_LOCAL_TTS);
        }
        return Err(format!("save local TTS switch: {error}"));
    }

    switch.set_enabled(enabled);

    // 关闭时卸载全部模型与引擎释放内存；重新启用时若 DeBERTa 已安装则重建引擎
    if enabled {
        if !local_state.engine.is_ready().await
            && local_state.paths.asset_present("deberta")
        {
            if let Err(e) = local_state.engine.init(&local_state.paths).await {
                tracing::error!("重新启用本地 TTS 时初始化引擎失败: {e}");
            }
        }
    } else {
        local_state.engine.unload_all().await;
    }

    Ok(LocalTtsSwitchStatus {
        configured_enabled: enabled,
        effective_enabled: enabled,
    })
}

/// 可用的推理设备（Windows DXGI / Linux Vulkan 枚举，复用 [`crate::utils::device::DeviceInfo`]）。
pub type InferenceDeviceInfo = crate::utils::device::DeviceInfo;

/// 获取当前推理设备（持久化配置或引擎实际值）。
#[tauri::command]
pub async fn tts_local_get_device(
    app: AppHandle,
    local_state: State<'_, LocalTtsState>,
) -> Result<String, String> {
    let engine_device = local_state.engine.device().await;
    // 优先返回持久化配置（与引擎一致）；未配置返回引擎当前值
    let configured = read_configured_device(&app).unwrap_or(engine_device);
    Ok(crate::utils::device::device_to_string(configured))
}

/// 枚举系统 DirectML 设备（委托 [`crate::utils::device::list_devices`]）。
#[tauri::command]
pub fn tts_local_list_devices() -> Vec<InferenceDeviceInfo> {
    crate::utils::device::list_devices()
}

/// 热切换本地 TTS 推理硬件设备。
/// 流程：保存配置 → 设置引擎 device → unload 全部 session → 若引擎已启用则重新 init。
/// 下次合成（或重新 init）时用新设备重建 session。
#[tauri::command]
pub async fn tts_local_set_device(
    app: AppHandle,
    local_state: State<'_, LocalTtsState>,
    device: String,
) -> Result<(), String> {
    let device = crate::utils::device::parse_device(&device)?;
    let device_str = crate::utils::device::device_to_string(device);

    // 保存配置；失败时回滚（与 set_enabled 行为一致）
    let store = config::settings_store(&app).map_err(|e| e.to_string())?;
    let previous = store.get(config::keys::LOCAL_TTS_DEVICE);
    store.set(config::keys::LOCAL_TTS_DEVICE, device_str.clone());
    if let Err(error) = store.save() {
        if let Some(value) = previous {
            store.set(config::keys::LOCAL_TTS_DEVICE, value);
        } else {
            store.delete(config::keys::LOCAL_TTS_DEVICE);
        }
        return Err(format!("保存推理设备失败: {error}"));
    }

    // 设置引擎 device + 卸载重建（热切换）
    local_state.engine.set_device(device).await;
    local_state.engine.unload_all().await;

    // 引擎已启用时重新初始化（用新设备）
    let enabled = load_configured_enabled(&app);
    if enabled && local_state.paths.asset_present("deberta") {
        if let Err(e) = local_state.engine.init(&local_state.paths).await {
            tracing::error!("切换推理设备后重新初始化引擎失败: {e}");
        }
    }

    tracing::info!("本地 TTS 推理设备已切换: {device_str}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri commands -- engine operations (from the former crate)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn tts_local_status(
    state: State<'_, LocalTtsState>,
) -> Result<TtsLocalStatus, String> {
    let voices = model_manager::list_voices(&state.paths)?;
    let deberta_installed = state.paths.asset_present("deberta");
    Ok(TtsLocalStatus {
        ready: state.engine.is_ready().await,
        deberta_installed,
        installed_voice_count: voices.len(),
    })
}

#[tauri::command]
pub async fn tts_local_list_catalog() -> Result<Vec<registry::AssetEntry>, String> {
    let all = registry::all_assets();
    // 收集所有被其他条目捆绑的资产 ID，在前端列表中隐藏它们
    let bundled: std::collections::HashSet<String> = all
        .iter()
        .flat_map(|a| a.bundled_assets.iter().cloned())
        .collect();
    Ok(all
        .into_iter()
        .filter(|a| !bundled.contains(&a.id))
        .collect())
}

#[tauri::command]
pub async fn tts_local_list_installed(
    state: State<'_, LocalTtsState>,
) -> Result<TtsLocalInstallSnapshot, String> {
    Ok(TtsLocalInstallSnapshot {
        assets: model_manager::list_assets(&state.paths)?,
        voices: model_manager::list_voices(&state.paths)?,
    })
}

// -- helpers ----------------------------------------------------------------

fn install_style_vectors_for(
    paths: &LocalTtsPaths,
    src: &Path,
    voice_id: &str,
) -> Result<PathBuf, String> {
    crate::utils::fs::copy_with_parent(src, &paths.voice_dir(voice_id).join("style_vectors.json"))
}

fn shared_asset_file_name(asset_id: &str) -> Result<&'static str, String> {
    match asset_id {
        "deberta" => Ok("deberta.onnx"),
        "deberta-tokenizer" => Ok("tokenizer.json"),
        other => Err(format!("unknown BERT asset: {other}")),
    }
}

fn download_temp_path(entry: &registry::AssetEntry, cache: &Path) -> PathBuf {
    let ext = registry::expected_extension(entry);
    cache.join(format!("{}.download.{ext}", entry.id))
}

fn default_voice_id(
    _inspected: &package::InspectedPackage,
    src: &Path,
) -> String {
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("voice");
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_lowercase();
    if cleaned.is_empty() {
        "voice".into()
    } else {
        cleaned
    }
}

// -- import / download / delete ---------------------------------------------

#[tauri::command]
pub async fn tts_local_import_from_path(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    path: String,
    voice_id: Option<String>,
) -> Result<ImportResult, String> {
    let (src, cleanup_after_import) =
        saf_bridge::prepare_file_import_source(&app, &path).await?;

    let result: std::result::Result<ImportResult, String> = async {
        if !src.exists() {
            return Err(format!("path not found: {}", src.display()));
        }
        let inspected = package::inspect_package(&src)?;
        // 角色语音只支持直接导入 .sbv2 / .onnx 原始模型文件，不再接受 zip/7z 压缩包
        if matches!(
            inspected.kind,
            package::PackageKind::Zip | package::PackageKind::SevenZ
        ) {
            return Err(
                "voice import does not support archives; import a .sbv2 or .onnx file directly"
                    .into(),
            );
        }
        let voice_id = match voice_id {
            Some(v) => v,
            None => default_voice_id(&inspected, &src),
        };
        let installed =
            package::install_inspected(&inspected, &src, &state.paths, &voice_id)?;
        let bytes = std::fs::metadata(&installed)
            .map(|m| m.len())
            .unwrap_or(0);
        let _ = app.emit("tts://install-complete", &voice_id);
        Ok(ImportResult {
            asset_id: voice_id.clone(),
            voice_id: Some(voice_id),
            path: installed.to_string_lossy().into_owned(),
            bytes,
            message: "imported".into(),
        })
    }
    .await;

    if cleanup_after_import {
        let _ = tokio::fs::remove_file(&src).await;
    }
    result
}

#[tauri::command]
pub async fn tts_local_download(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    asset_id: String,
) -> Result<Vec<ImportResult>, String> {
    let entry = registry::find(&asset_id)
        .ok_or_else(|| format!("asset {asset_id} not in catalog"))?;

    let cancel = Arc::new(CancellationToken::new());
    {
        let mut guard = state.cancel.lock().await;
        *guard = Some(cancel.clone());
    }

    // 收集所有需要下载的资产：主资产 + 捆绑资产
    let bundled_ids = entry.bundled_assets.clone();
    let mut to_download: Vec<registry::AssetEntry> = vec![entry];
    for bundled_id in &bundled_ids {
        if let Some(e) = registry::find(bundled_id) {
            to_download.push(e);
        }
    }

    let result = async {
        let mut results: Vec<ImportResult> = Vec::new();
        for entry in &to_download {
            let r = download_single_asset(&app, &state, entry, cancel.clone()).await?;
            results.push(r);
        }
        // DeBERTa 全套就位时初始化引擎
        if state.paths.asset_present("deberta") {
            let _ = state.engine.init(&state.paths).await;
        }
        Ok::<_, String>(results)
    }
    .await;

    {
        let mut guard = state.cancel.lock().await;
        *guard = None;
    }
    let _ = app.emit("tts://download-complete", &asset_id);
    result
}

/// 下载单个资产（Bert/Voice/StyleVectors），返回 ImportResult。
async fn download_single_asset(
    app: &AppHandle,
    state: &LocalTtsState,
    entry: &registry::AssetEntry,
    cancel: Arc<CancellationToken>,
) -> Result<ImportResult, String> {
    match entry.kind {
        registry::AssetKind::Bert => {
            let file_name = shared_asset_file_name(&entry.id)?;
            let dst = state.paths.deberta_dir().join(file_name);
            std::fs::create_dir_all(state.paths.deberta_dir())
                .map_err(|e| format!("mkdir deberta: {e}"))?;
            let bytes = download::download_asset(app, entry, &dst, cancel).await?;
            Ok(ImportResult {
                asset_id: entry.id.clone(),
                voice_id: None,
                path: dst.to_string_lossy().into_owned(),
                bytes,
                message: format!("{} downloaded", entry.id),
            })
        }
        registry::AssetKind::Voice => {
            let raw_dst = download_temp_path(entry, &state.paths.cache);
            let bytes = download::download_asset(app, entry, &raw_dst, cancel).await?;
            let inspected = package::inspect_package(&raw_dst)?;
            let installed = package::install_inspected(
                &inspected,
                &raw_dst,
                &state.paths,
                &entry.id,
            )?;
            let _ = tokio::fs::remove_file(&raw_dst).await;
            Ok(ImportResult {
                asset_id: entry.id.clone(),
                voice_id: Some(entry.id.clone()),
                path: installed.to_string_lossy().into_owned(),
                bytes,
                message: "voice downloaded".into(),
            })
        }
        registry::AssetKind::StyleVectors => {
            let voice_id = entry.voice_id.clone().ok_or_else(|| {
                format!("style_vectors asset {} missing voice_id", entry.id)
            })?;
            let raw_dst = download_temp_path(entry, &state.paths.cache);
            let bytes = download::download_asset(app, entry, &raw_dst, cancel).await?;
            let installed =
                install_style_vectors_for(&state.paths, &raw_dst, &voice_id)?;
            let _ = tokio::fs::remove_file(&raw_dst).await;
            Ok(ImportResult {
                asset_id: entry.id.clone(),
                voice_id: Some(voice_id.clone()),
                path: installed.to_string_lossy().into_owned(),
                bytes,
                message: "style vectors downloaded".into(),
            })
        }
        registry::AssetKind::SherpaOnnx => {
            let model_dir = state.paths.sherpa_onnx_models_dir();
            std::fs::create_dir_all(&model_dir)
                .map_err(|e| format!("mkdir sherpa_onnx_models: {e}"))?;
            let dest_dir = model_dir.join(&entry.id);
            if dest_dir.exists() {
                std::fs::remove_dir_all(&dest_dir)
                    .map_err(|e| format!("remove existing model dir: {e}"))?;
            }

            // 计划 A：ModelScope 整目录下载；计划 B：HF Mirror 逐文件下载；计划 C：GitHub tar.bz2 整包回退
            let bytes = if let Some(mdir) = &entry.modelscope_dir {
                match download_sherpa_modelscope_dir(app, entry, mdir, &dest_dir, cancel.clone())
                    .await
                {
                    Ok(bytes) => bytes,
                    Err(ms_err) => {
                        tracing::warn!(
                            "ModelScope download failed for {}: {ms_err}, trying GitHub fallback",
                            entry.id
                        );
                        if dest_dir.exists() {
                            let _ = std::fs::remove_dir_all(&dest_dir);
                        }
                        download_sherpa_github_fallback(
                            app,
                            entry,
                            &dest_dir,
                            &state.paths.cache,
                            cancel.clone(),
                        )
                            .await
                            .map_err(|gh_err| {
                                format!(
                                    "ModelScope failed: {ms_err}; GitHub fallback also failed: {gh_err}"
                                )
                            })?
                    }
                }
            } else if !entry.hf_files.is_empty() {
                match download_sherpa_hf_files(app, entry, &dest_dir, cancel.clone()).await {
                    Ok(bytes) => bytes,
                    Err(hf_err) => {
                        tracing::warn!(
                            "HF mirror download failed for {}: {hf_err}, trying GitHub fallback",
                            entry.id
                        );
                        if dest_dir.exists() {
                            let _ = std::fs::remove_dir_all(&dest_dir);
                        }
                        download_sherpa_github_fallback(
                            app,
                            entry,
                            &dest_dir,
                            &state.paths.cache,
                            cancel.clone(),
                        )
                            .await
                            .map_err(|gh_err| {
                                format!(
                                    "HF failed: {hf_err}; GitHub fallback also failed: {gh_err}"
                                )
                            })?
                    }
                }
            } else {
                // 无 hf_files 时走原有 tar.bz2 逻辑
                let raw_dst = download_temp_path(entry, &state.paths.cache);
                let bytes =
                    download::download_asset(app, entry, &raw_dst, cancel.clone()).await?;
                std::fs::create_dir_all(&dest_dir)
                    .map_err(|e| format!("mkdir model dest: {e}"))?;
                extract_archive(&raw_dst, &dest_dir)
                    .map_err(|e| format!("extract archive: {e}"))?;
                let _ = tokio::fs::remove_file(&raw_dst).await;
                bytes
            };

            let _ = app.emit("tts://sherpa-onnx-model-downloaded", &entry.id);
            Ok(ImportResult {
                asset_id: entry.id.clone(),
                voice_id: None,
                path: dest_dir.to_string_lossy().into_owned(),
                bytes,
                message: format!("{} downloaded", entry.display_name),
            })
        }
    }
}

#[tauri::command]
pub async fn tts_local_delete_voice(
    state: State<'_, LocalTtsState>,
    voice_id: String,
) -> Result<(), String> {
    model_manager::delete_voice(&state.paths, &voice_id)
}

/// 删除 DeBERTa 共享模型（deberta.onnx + tokenizer.json），并卸载引擎释放内存。
/// 删除后可从模型下载目录重新下载。
#[tauri::command]
pub async fn tts_local_delete_deberta(
    state: State<'_, LocalTtsState>,
) -> Result<(), String> {
    delete_deberta_asset(&state.paths)?;
    state.engine.unload_all().await;
    Ok(())
}

/// 移除 `assets/deberta` 目录（幂等：目录不存在时静默成功）。
fn delete_deberta_asset(paths: &LocalTtsPaths) -> Result<(), String> {
    let dir = paths.deberta_dir();
    crate::utils::path::validate_path_in_base(&dir, &paths.assets)?;
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("remove_dir_all: {e}"))?;
    }
    Ok(())
}

/// 解压归档文件到目标目录（支持 tar.bz2、zip）。
///
/// 解压后如果 `dest` 内恰好只有一个子目录且无文件，则将该子目录的内容
/// 上提一级，避免 k2-fsa 等源的 tar.bz2 包自带顶层目录导致路径嵌套。
fn extract_archive(src: &Path, dest: &Path) -> Result<(), String> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let src_display = src.to_string_lossy().to_string();

    match ext.as_str() {
        "bz2" => {
            let f = std::fs::File::open(&src_display)
                .map_err(|e| format!("open tar.bz2: {e}"))?;
            let dec = bzip2::read::BzDecoder::new(f);
            let mut archive = tar::Archive::new(dec);
            archive
                .unpack(dest)
                .map_err(|e| format!("untar bz2: {e}"))?;
        }
        "tar" => {
            let f = std::fs::File::open(&src_display)
                .map_err(|e| format!("open tar: {e}"))?;
            let mut archive = tar::Archive::new(f);
            archive
                .unpack(dest)
                .map_err(|e| format!("untar: {e}"))?;
        }
        "gz" => {
            let f = std::fs::File::open(&src_display)
                .map_err(|e| format!("open tar.gz: {e}"))?;
            let dec = flate2::read::GzDecoder::new(f);
            let mut archive = tar::Archive::new(dec);
            archive
                .unpack(dest)
                .map_err(|e| format!("untar gz: {e}"))?;
        }
        "zip" => {
            let f = std::fs::File::open(&src_display)
                .map_err(|e| format!("open zip: {e}"))?;
            let mut zip = zip::ZipArchive::new(f)
                .map_err(|e| format!("open zip archive: {e}"))?;
            zip.extract(dest)
                .map_err(|e| format!("extract zip: {e}"))?;
        }
        other => {
            return Err(format!("unsupported archive format: {other}"));
        }
    }

    // Flatten: if dest contains exactly one subdirectory and zero files,
    // move the subdirectory's contents up to dest.
    flatten_single_subdir(dest)?;

    Ok(())
}

/// 从 HF Mirror 逐文件下载 Sherpa-ONNX 模型到目标目录。
/// 成功返回 Ok(总字节数)；任一文件失败则返回 Err，由调用方决定回退策略。
async fn download_sherpa_hf_files(
    app: &AppHandle,
    entry: &registry::AssetEntry,
    dest_dir: &Path,
    cancel: Arc<CancellationToken>,
) -> Result<u64, String> {
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| format!("mkdir sherpa model dir: {e}"))?;

    let client = download_client();
    let on_progress = sherpa_progress_cb(app, &entry.id);
    let mut total_bytes: u64 = 0;

    for (rel_path, url) in &entry.hf_files {
        let file_dest = dest_dir.join(rel_path);
        if let Some(parent) = file_dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir for {rel_path}: {e}"))?;
        }
        let bytes = crate::utils::download::download_to_file(
            client,
            url,
            &file_dest,
            Some(cancel.clone()),
            Some(on_progress.clone()),
            0,
        )
        .await
        .map_err(|e| format!("download {rel_path}: {e}"))?;
        total_bytes += bytes;
    }
    Ok(total_bytes)
}

/// 将 `<repo>/<子目录>` 拆分为仓库与子目录。
///
/// ModelScope 仓库路径形如 `<org>/<name>`（如 `gomodels/sherpa`），因此整个
/// `mdir`（如 `gomodels/sherpa/matcha-icefall-zh-baker`）按**最后一个** `/` 拆分，
/// 而不是第一个——否则 `repo` 会变成 `gomodels`，API 与 resolve URL 全部 404。
fn split_modelscope_dir(mdir: &str) -> Result<(&str, &str), String> {
    mdir.rsplit_once('/')
        .ok_or_else(|| format!("invalid modelscope_dir: {mdir}"))
}

/// 从 ModelScope 仓库按目录整目录下载 Sherpa-ONNX 模型到目标目录。
///
/// `mdir` 形如 `<repo>/<子目录>`（如 `gomodels/sherpa/matcha-icefall-zh-baker`）。
/// 优先通过 ModelScope repo API 递归枚举目录内全部 blob，再逐一下载到 `dest_dir`，
/// 保留相对路径（含 espeak-ng-data、dict 等嵌套目录）；API 不可用时回退到
/// [`registry::static_subdir_files`] 内置清单，逻辑对调用方透明。成功返回总字节数。
async fn download_sherpa_modelscope_dir(
    app: &AppHandle,
    entry: &registry::AssetEntry,
    mdir: &str,
    dest_dir: &Path,
    cancel: Arc<CancellationToken>,
) -> Result<u64, String> {
    std::fs::create_dir_all(dest_dir)
        .map_err(|e| format!("mkdir sherpa model dir: {e}"))?;

    let (repo, subdir) = split_modelscope_dir(mdir)?;

    let client = download_client();

    // 1) 先试 ModelScope repo API 枚举目录（www → apex，部分网络会把 apex 域名
    //    的 `/api/**` 路由劫持为 `404 page not found`）；2) 全挂时回退到内置
    //    静态清单直接按文件名从 `www.modelscope.cn/models/.../resolve/master/` 拉取，
    //    让下载仍然走 ModelScope 而不是直接掉回 GitHub。
    let prefix = format!("{subdir}/");
    let paths: Vec<String> = match fetch_modelscope_dir_paths(client, repo, &prefix).await {
        Ok(list) if !list.is_empty() => list,
        Ok(_) => {
            return Err(format!("ModelScope 目录为空或不存在: {mdir}"));
        }
        Err(api_err) => match registry::static_subdir_files(&entry.id) {
            Some(static_files) => {
                tracing::warn!(
                    "ModelScope repo API 不可用: {api_err}; 改用内置静态清单下载 {}（{} 个文件）",
                    entry.id,
                    static_files.len()
                );
                static_files
                    .iter()
                    .map(|rel| format!("{subdir}/{rel}"))
                    .collect()
            }
            None => return Err(api_err),
        },
    };

    let on_progress = sherpa_progress_cb(app, &entry.id);
    let mut total_bytes: u64 = 0;
    let mut count: usize = 0;

    for path in &paths {
        if cancel.is_cancelled() {
            return Err("download cancelled".into());
        }
        let rel_path = &path[prefix.len()..];
        let url = format!(
            "https://www.modelscope.cn/models/{repo}/resolve/master/{}",
            url_encode_path(path)
        );
        let bytes = crate::utils::download::download_to_file(
            client,
            &url,
            &dest_dir.join(rel_path),
            Some(cancel.clone()),
            Some(on_progress.clone()),
            0,
        )
        .await
        .map_err(|e| format!("download {path}: {e}"))?;
        total_bytes += bytes;
        count += 1;
    }

    if count == 0 {
        return Err(format!("ModelScope 目录为空或不存在: {mdir}"));
    }
    Ok(total_bytes)
}

/// 尝试通过 ModelScope repo API 列出仓库 `<子目录>/` 下全部 blob 的完整路径。
///
/// 先试 `www.` 域名再试 apex（部分网络环境下 apex 的 `/api/**` 路由会被劫持为
/// `404 page not found`），全部失败时返回聚合错误。成功返回按仓库根的完整
/// 文件路径列表（形如 `<subdir>/dict/user.dict.utf8`）。
async fn fetch_modelscope_dir_paths(
    client: &reqwest::Client,
    repo: &str,
    prefix: &str,
) -> Result<Vec<String>, String> {
    let mut api_errors = String::new();
    for host in ["https://www.modelscope.cn", "https://modelscope.cn"] {
        let api_url =
            format!("{host}/api/v1/models/{repo}/repo/files?Revision=master&Recursive=true");
        let list = match fetch_modelscope_file_list(client, &api_url).await {
            Ok(list) => list,
            Err(e) => {
                api_errors.push_str(&format!("{host}: {e}; "));
                continue;
            }
        };
        let mut paths = Vec::new();
        for item in &list {
            if item["Type"].as_str() != Some("blob") {
                continue;
            }
            let Some(path) = item["Path"].as_str() else {
                continue;
            };
            if path.starts_with(prefix) && path.len() > prefix.len() {
                paths.push(path.to_string());
            }
        }
        return Ok(paths);
    }
    Err(format!("ModelScope repo API 全部失败: {api_errors}"))
}

/// 对 ModelScope 文件路径做 URL 百分号编码。
///
/// 仓库里个别文件名为带空格（如 `espeak-ng-data/voices/!v/Mr serious`），
/// reqwest 不会自动编码空格，直接拼接 URL 会得到 404/400。
/// 这里仅保留 RFC 3986 非保留字符（`A-Z a-z 0-9 - _ . ~`）和 `/`，其余一律编码。
fn url_encode_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 16);
    for b in path.bytes() {
        // 仅保留 RFC 3986 非保留字符和路径分隔符，其余一律百分号编码。
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// 请求一次 ModelScope repo 文件列表 API，成功返回 `Data.Files` 数组。
async fn fetch_modelscope_file_list(
    client: &reqwest::Client,
    api_url: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let resp = client
        .get(api_url)
        .send()
        .await
        .map_err(|e| format!("request: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("read body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "HTTP {status}: {}",
            text.chars().take(512).collect::<String>()
        ));
    }
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("parse response: {e}"))?;
    json["Data"]["Files"]
        .as_array()
        .cloned()
        .ok_or_else(|| format!("missing Data.Files field"))
}

/// 从 GitHub Releases 下载 tar.bz2 回退包并解压到目标目录。
/// 临时归档文件放在 `cache` 目录（卸载/清理后不残留到 tts-local 根目录）。
async fn download_sherpa_github_fallback(
    app: &AppHandle,
    entry: &registry::AssetEntry,
    dest_dir: &Path,
    cache: &Path,
    cancel: Arc<CancellationToken>,
) -> Result<u64, String> {
    let fallback_url = entry
        .github_fallback_url
        .as_deref()
        .ok_or("no fallback URL available")?;

    let temp_path = cache.join(format!("{}.fallback.tar.bz2", entry.id));

    let client = download_client();
    let bytes = crate::utils::download::download_to_file(
        client,
        fallback_url,
        &temp_path,
        Some(cancel),
        Some(sherpa_progress_cb(app, &entry.id)),
        entry.size_bytes,
    )
    .await?;
    extract_archive(&temp_path, dest_dir)
        .map_err(|e| format!("extract fallback archive: {e}"))?;
    let _ = tokio::fs::remove_file(&temp_path).await;
    Ok(bytes)
}

/// 如果 `dir` 内恰好只有一个子目录且没有文件，则将子目录内容上提一级。
/// k2-fsa/sherpa-onnx 的 tar.bz2 包内通常有一层顶层目录，
/// 解压后形如 `dest/<model_name>/model.onnx`，需要展平为 `dest/model.onnx`。
fn flatten_single_subdir(dir: &Path) -> Result<(), String> {
    let entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir for flatten: {e}"))?
        .filter_map(|e| e.ok())
        .collect();

    if entries.len() != 1 {
        return Ok(());
    }

    let single = &entries[0];
    if !single.path().is_dir() {
        return Ok(());
    }

    let subdir = single.path();
    let sub_entries: Vec<_> = std::fs::read_dir(&subdir)
        .map_err(|e| format!("read subdir for flatten: {e}"))?
        .filter_map(|e| e.ok())
        .collect();

    // Only flatten if the subdirectory contains at least one entry
    if sub_entries.is_empty() {
        return Ok(());
    }

    for entry in &sub_entries {
        let dest = dir.join(entry.file_name());
        std::fs::rename(entry.path(), &dest)
            .map_err(|e| format!("flatten move {:?} -> {:?}: {e}", entry.path(), dest))?;
    }
    std::fs::remove_dir(&subdir)
        .map_err(|e| format!("flatten remove empty subdir: {e}"))?;
    Ok(())
}

#[tauri::command]
pub async fn tts_local_import_style_vectors(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    voice_id: String,
    path: String,
) -> Result<ImportResult, String> {
    if voice_id.is_empty() || voice_id.len() > 64 {
        return Err("voice id length out of range".into());
    }
    if !voice_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("voice id must be kebab-case ASCII".into());
    }

    let voice_dir = state.paths.voice_dir(&voice_id);
    if !voice_dir.exists() {
        return Err(format!(
            "voice {voice_id} not found; import the .onnx or .sbv2 model first"
        ));
    }
    if voice_dir.join("model.sbv2").exists() {
        return Err(format!(
            "voice {voice_id} is .sbv2 form; style vectors are embedded and cannot be replaced"
        ));
    }

    let (src, cleanup_after_import) =
        saf_bridge::prepare_file_import_source(&app, &path).await?;
    let result: std::result::Result<ImportResult, String> = async {
        if !src.exists() {
            return Err(format!("path not found: {path}"));
        }
        let destination = state.paths.style_vectors_path(&voice_id);
        std::fs::copy(&src, &destination)
            .map_err(|e| format!("copy style_vectors.json: {e}"))?;
        let bytes = std::fs::metadata(&destination).map(|m| m.len()).unwrap_or(0);
        let _ = app.emit("tts://install-complete", &voice_id);
        Ok(ImportResult {
            asset_id: voice_id.clone(),
            voice_id: Some(voice_id),
            path: destination.to_string_lossy().into_owned(),
            bytes,
            message: "style vectors imported".into(),
        })
    }
    .await;

    if cleanup_after_import {
        let _ = tokio::fs::remove_file(&src).await;
    }
    result
}

#[tauri::command]
pub async fn tts_local_synthesize_preview(
    state: State<'_, LocalTtsState>,
    text: String,
    voice_id: String,
    length_scale: f32,
    sdp_ratio: f32,
) -> Result<Response, String> {
    if !state.engine.is_ready().await {
        return Err(
            "local TTS engine not initialized (missing DeBerta)".into()
        );
    }
    state.engine.load_voice(&state.paths, &voice_id).await?;
    let req = SynthesizeRequest {
        voice_id,
        text,
        style_id: 0,
        speaker_id: 0,
        sdp_ratio,
        length_scale,
    };
    state.engine.synthesize(req).await.map(wav_response)
}

fn wav_response(bytes: Vec<u8>) -> Response {
    Response::new(bytes)
}

// ---------------------------------------------------------------------------
// Sherpa-ONNX model management commands
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct SherpaOnnxModelRecord {
    pub id: String,
    pub display_name: String,
    pub model_type: String,
    pub language: String,
    pub voice: String,
    pub size_bytes: u64,
    pub path: String,
    pub installed: bool,
}

/// 列出所有已下载的 Sherpa-ONNX 模型
#[tauri::command]
pub async fn tts_local_list_sherpa_models(
    state: State<'_, LocalTtsState>,
) -> Result<Vec<SherpaOnnxModelRecord>, String> {
    let model_dir = state.paths.sherpa_onnx_models_dir();
    let catalog = registry::all_assets();
    let mut records: Vec<SherpaOnnxModelRecord> = Vec::new();

    for entry in &catalog {
        if entry.kind != registry::AssetKind::SherpaOnnx {
            continue;
        }
        let model_path = model_dir.join(&entry.id);
        let installed = model_path.exists();
        let meta = entry.sherpa_meta.as_ref();
        let size = if installed {
            dir_size(&model_path).unwrap_or(0)
        } else {
            0
        };
        records.push(SherpaOnnxModelRecord {
            id: entry.id.clone(),
            display_name: entry.display_name.clone(),
            model_type: meta.map(|m| m.model_type.clone()).unwrap_or_default(),
            language: meta.map(|m| m.language.clone()).unwrap_or_default(),
            voice: meta.map(|m| m.voice.clone()).unwrap_or_default(),
            size_bytes: size,
            path: model_path.to_string_lossy().into_owned(),
            installed,
        });
    }

    Ok(records)
}

/// 下载指定的 Sherpa-ONNX 模型（从 registry 目录）
#[tauri::command]
pub async fn tts_local_download_sherpa_model(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    model_id: String,
) -> Result<ImportResult, String> {
    tts_local_download(app, state, model_id).await
        .map(|results| results.into_iter().next().unwrap_or_else(|| ImportResult {
            asset_id: String::new(),
            voice_id: None,
            path: String::new(),
            bytes: 0,
            message: "no result".into(),
        }))
}

/// 删除指定的 Sherpa-ONNX 模型
#[tauri::command]
pub async fn tts_local_delete_sherpa_model(
    state: State<'_, LocalTtsState>,
    model_id: String,
) -> Result<(), String> {
    let model_dir = state.paths.sherpa_onnx_models_dir().join(&model_id);
    crate::utils::path::validate_path_in_base(&model_dir, &state.paths.sherpa_onnx_models_dir())?;
    if model_dir.exists() {
        std::fs::remove_dir_all(&model_dir)
            .map_err(|e| format!("remove sherpa model: {e}"))?;
    }
    Ok(())
}

/// 递归计算目录大小
fn dir_size(path: &std::path::Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total += dir_size(&entry.path())?;
        } else {
            total += meta.len();
        }
    }
    Ok(total)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::ipc::{InvokeResponseBody, IpcResponse};

    fn test_paths(root: &std::path::Path) -> LocalTtsPaths {
        LocalTtsPaths {
            root: root.join("models").join("tts-local"),
            assets: root.join("models").join("tts-local").join("assets"),
            voices: root.join("models").join("tts-local").join("voices"),
            cache: root.join("cache"),
        }
    }

    #[test]
    fn url_encode_path_encodes_spaces_and_specials_only() {
        assert_eq!(
            url_encode_path("matcha_tts_zh_en_20251010/espeak-ng-data/voices/!v/Mr serious"),
            "matcha_tts_zh_en_20251010/espeak-ng-data/voices/%21v/Mr%20serious"
        );
        assert_eq!(
            url_encode_path("a#b?c&d+e  f"),
            "a%23b%3Fc%26d%2Be%20%20f"
        );
        assert_eq!(url_encode_path("plain/model.onnx"), "plain/model.onnx");
    }

    #[test]
    fn split_modelscope_dir_keeps_org_in_repo() {
        assert_eq!(
            split_modelscope_dir("gomodels/sherpa/matcha-icefall-zh-baker").unwrap(),
            ("gomodels/sherpa", "matcha-icefall-zh-baker")
        );
        assert_eq!(
            split_modelscope_dir("lingchat-research-studio/DeBERTa.onnx").unwrap(),
            ("lingchat-research-studio", "DeBERTa.onnx")
        );
        assert!(split_modelscope_dir("no-slash").is_err());
    }

    #[test]
    fn local_tts_switch_can_be_changed_at_runtime() {
        let switch = LocalTtsSwitch::new(false);
        assert!(!switch.is_enabled());
        switch.set_enabled(true);
        assert!(switch.is_enabled());
        switch.set_enabled(false);
        assert!(!switch.is_enabled());
    }

    #[test]
    fn preview_wav_uses_raw_ipc_response() {
        let response = wav_response(vec![0x52, 0x49, 0x46, 0x46]);
        match response.body().unwrap() {
            InvokeResponseBody::Raw(bytes) => assert_eq!(bytes, b"RIFF"),
            InvokeResponseBody::Json(_) => panic!("preview WAV was JSON serialized"),
        }
    }

    #[test]
    fn shared_asset_download_uses_individual_canonical_file_names() {
        assert_eq!(shared_asset_file_name("deberta").unwrap(), "deberta.onnx");
        assert_eq!(
            shared_asset_file_name("deberta-tokenizer").unwrap(),
            "tokenizer.json"
        );
        assert!(shared_asset_file_name("unknown").is_err());
    }

    #[test]
    fn delete_deberta_removes_asset_dir_and_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        std::fs::create_dir_all(paths.deberta_dir()).unwrap();
        std::fs::write(paths.deberta_dir().join("deberta.onnx"), b"model").unwrap();
        std::fs::write(paths.deberta_dir().join("tokenizer.json"), b"{}").unwrap();

        delete_deberta_asset(&paths).unwrap();
        assert!(!paths.deberta_dir().exists());

        // 幂等：目录不存在时也不报错
        delete_deberta_asset(&paths).unwrap();
    }

    #[test]
    fn download_temp_path_preserves_catalog_extension() {
        let cache = Path::new("C:/tts-cache");
        let voice = registry::find("ling-v2").unwrap();
        let style = registry::find("ling-v2-style").unwrap();
        assert_eq!(
            download_temp_path(&voice, cache),
            PathBuf::from("C:/tts-cache/ling-v2.download.onnx")
        );
        assert_eq!(
            download_temp_path(&style, cache),
            PathBuf::from("C:/tts-cache/ling-v2-style.download.json")
        );
    }

    #[test]
    fn style_vectors_resolves_to_voice_directory() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let source = temp.path().join("downloaded.json");
        std::fs::write(&source, b"{\"v\":1}").unwrap();

        let installed = install_style_vectors_for(&paths, &source, "ling-v2").unwrap();
        let expected = paths.voice_dir("ling-v2").join("style_vectors.json");
        assert_eq!(installed, expected);
        assert_eq!(std::fs::read(installed).unwrap(), b"{\"v\":1}");
    }
}
