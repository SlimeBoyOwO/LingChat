//! Sherpa-ONNX 本地 TTS 模型管理：列表、下载、删除、试听合成。
//!
//! 模型目录固定为 `data/sherpa_onnx_models/<model_name>/`，与
//! `ai_service::tts::local::sherpa_onnx_manager::SherpaOnnxManager` 一致。
//! 角色把 `tts_type` 设为 `sherpa-onnx`/`sherpa` 并选择模型名后即可走该路径合成。

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, RwLock};

use crate::ai_service::tts::adapters::sherpa_onnx::SherpaOnnxAdapter;
use crate::ai_service::tts::adapters::sherpa_onnx::load_reference_audio as load_ref_audio;
use crate::ai_service::tts::local::sherpa_onnx_manager::{SherpaOnnxManager, SherpaOnnxModelInfo};
use crate::ai_service::tts::provider::TtsAdapter;

/// 前端模型记录。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SherpaModelRecord {
    pub id: String,
    pub display_name: String,
    pub model_type: String,
    pub language: String,
    pub voice: String,
    pub size_bytes: u64,
    pub installed: bool,
    pub path: String,
}

/// Sherpa 模型管理器句柄（供命令使用）。
#[derive(Default)]
pub struct SherpaModelsState {
    pub manager: Arc<RwLock<Option<Arc<SherpaOnnxManager>>>>,
    pub downloading: Arc<Mutex<std::collections::HashSet<String>>>,
}

/// 在 setup 中把 Sherpa 模型管理器挂到全局。
pub fn init() -> SherpaModelsState {
    let manager = Arc::new(SherpaOnnxManager::new(
        crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root(),
    ));
    SherpaModelsState {
        manager: Arc::new(RwLock::new(Some(manager))),
        downloading: Arc::new(Mutex::new(Default::default())),
    }
}

/// 获取当前管理器（懒解析数据目录，若尚未初始化则就地创建）。
async fn get_manager(
    state: &State<'_, SherpaModelsState>,
) -> Result<Arc<SherpaOnnxManager>, String> {
    let mut guard = state.manager.write().await;
    if guard.is_none() {
        let mgr = Arc::new(SherpaOnnxManager::new(
            crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root(),
        ));
        mgr.initialize()
            .await
            .map_err(|e| format!("Sherpa 模型管理器初始化失败: {e}"))?;
        *guard = Some(mgr.clone());
        Ok(mgr)
    } else {
        Ok(guard.as_ref().unwrap().clone())
    }
}

fn record_from(info: &SherpaOnnxModelInfo) -> SherpaModelRecord {
    SherpaModelRecord {
        id: info.model_name.clone(),
        display_name: info.model_name.clone(),
        model_type: info.model_type.clone(),
        language: info.language.clone(),
        voice: info.voice.clone(),
        size_bytes: info.size_bytes,
        installed: true,
        path: info.model_path.to_string_lossy().to_string(),
    }
}

/// 列出 Sherpa 模型（内置清单 + 已安装状态）。
#[tauri::command]
pub async fn sherpa_list_models(
    state: State<'_, SherpaModelsState>,
) -> Result<Vec<SherpaModelRecord>, String> {
    let mgr = get_manager(&state).await?;
    mgr.initialize()
        .await
        .map_err(|e| format!("扫描模型失败: {e}"))?;
    let models = mgr.get_models().await;
    let mut installed: std::collections::HashMap<String, SherpaModelRecord> = models
        .iter()
        .map(record_from)
        .map(|r| (r.id.clone(), r))
        .collect();

    let mut records = sherpa_catalog();
    // 未安装的清单模型：拉取仓库大小（用于展示“模型大小”）
    let mut size_tasks = Vec::new();
    for rec in records.iter().filter(|r| !installed.contains_key(&r.id)) {
        let dir = rec.id.clone();
        let dir_for_size = rec.id.clone();
        size_tasks.push(async move { (dir, repo_dir_total_bytes(&dir_for_size).await) });
    }
    let sizes: std::collections::HashMap<String, u64> = futures_util::future::join_all(size_tasks)
        .await
        .into_iter()
        .collect();

    for rec in records.iter_mut() {
        if let Some(inst) = installed.remove(&rec.id) {
            *rec = inst;
        } else {
            rec.installed = false;
            if let Some(&size) = sizes.get(&rec.id) {
                rec.size_bytes = size;
            }
        }
    }
    // 追加目录里存在但不在内置清单中的模型（自动识别导入的模型）
    for (_, inst) in installed {
        records.push(inst);
    }
    Ok(records)
}

/// 删除一个已安装的 Sherpa 模型（删除整个模型目录）。
#[tauri::command]
pub async fn sherpa_delete_model(
    state: State<'_, SherpaModelsState>,
    model_id: String,
) -> Result<(), String> {
    let mgr = get_manager(&state).await?;
    // model_id 只允许是模型目录下的单个普通目录名：拒绝路径分隔符与 `.`/`..`，
    // 防止把删除操作引导到模型目录之外（路径穿越）。
    if model_id.is_empty()
        || model_id == "."
        || model_id == ".."
        || model_id.contains('/')
        || model_id.contains('\\')
    {
        return Err(format!("非法模型 ID: {model_id}"));
    }
    let dir = mgr.model_dir().join(&model_id);
    if !dir.exists() {
        return Err(format!("Sherpa 模型不存在: {model_id}"));
    }
    tokio::fs::remove_dir_all(&dir)
        .await
        .map_err(|e| format!("删除模型 {model_id} 失败: {e}"))?;
    let _ = mgr.initialize().await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Sherpa 模型清单（下载源 github/gomodels/sherpa，含全部 TTS 模型）
// ---------------------------------------------------------------------------

const SHERPA_REPO_FILES_API: &str = "https://www.modelscope.cn/api/v1/models/gomodels/sherpa/repo/files?Revision=master&Recursive=true";

const SHERPA_FILE_DL: &str = "https://www.modelscope.cn/models/gomodels/sherpa/resolve/master";

/// 可下载的 Sherpa TTS 模型（repo 目录 → 适配器所需 model_type）。
fn sherpa_catalog_defs() -> Vec<(String, String, String, String, String)> {
    vec![
        // (dir, display_name, language, voice, model_type)
        (
            "kokoro-int8-multi-lang-v1_1".into(),
            "Kokoro 多语言 (int8)".into(),
            "zh".into(),
            "female".into(),
            "kokoro".into(),
        ),
        (
            "matcha-icefall-zh-baker".into(),
            "Matcha 中文 (新汉语真人) · icefall".into(),
            "zh".into(),
            "female".into(),
            "matcha".into(),
        ),
        (
            "matcha_tts_zh_en_20251010".into(),
            "Matcha 中英 (MMTTS)".into(),
            "zh".into(),
            "female".into(),
            "matcha".into(),
        ),
        (
            "sherpa-onnx-zipvoice-distill-zh-en-emilia".into(),
            "ZipVoice 中英 (Emilia)".into(),
            "zh".into(),
            "female".into(),
            "zipvoice".into(),
        ),
    ]
}

/// 内置可下载模型的元数据（含下载仓库估价，用于列表展示）。
pub fn sherpa_catalog() -> Vec<SherpaModelRecord> {
    sherpa_catalog_defs()
        .into_iter()
        .map(|(dir, name, lang, voice, mtype)| SherpaModelRecord {
            id: dir,
            display_name: name,
            model_type: mtype,
            language: lang,
            voice,
            size_bytes: 0,
            installed: false,
            path: String::new(),
        })
        .collect()
}

/// 各模型目录需跳过的冗余文件（节省流量；onnx 为推理唯一所需）。
/// - int8 vits 变体仅在无 int8 时才需要；有非 int8 则跳过 int8。
/// - pytorch_model*.bin 与 .wav（示例音频）不是推理必需。
fn skip_repo_file(dir: &str, path: &str) -> bool {
    if path.ends_with("pytorch_model.bin") || path.ends_with("pytorch_model_sherpa.bin") {
        return true;
    }
    if path.ends_with(".wav") || path.ends_with(".gitattributes") {
        return true;
    }
    // ZipVoice：若存在非 int8 的 encoder/decoder，则跳过 int8 冗余
    if dir == "sherpa-onnx-zipvoice-distill-zh-en-emilia"
        && (path.ends_with("fm_decoder_int8.onnx") || path.ends_with("text_encoder_int8.onnx"))
    {
        return true;
    }
    false
}

#[derive(Debug, Clone)]
struct RepoFile {
    rel: String, // 相对模型目录的路径，如 espeak-ng-data/lang/zh/zh_list
    size: u64,
}

/// 从 modelscope 文件 API 拉取某模型目录下的文件清单（过滤目录项 / 跳过项）。
async fn fetch_repo_files(dir: &str) -> Result<Vec<RepoFile>, String> {
    let client = crate::utils::download::build_download_client()
        .map_err(|e| format!("创建下载客户端失败: {e}"))?;
    let resp = client
        .get(SHERPA_REPO_FILES_API)
        .send()
        .await
        .map_err(|e| format!("请求模型清单失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("模型清单 HTTP {}", resp.status()));
    }
    let body = resp
        .text()
        .await
        .map_err(|e| format!("读取清单失败: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let prefix = format!("{}/", dir);
    let mut out = Vec::new();
    if let Some(files) = value.pointer("/Data/Files").and_then(|f| f.as_array()) {
        for f in files {
            let path = f.get("Path").and_then(|v| v.as_str()).unwrap_or_default();
            if !path.starts_with(&prefix) {
                continue;
            }
            // 只要文件（is_lfs 或无 Type=tree）；Size==0 且 Name 无拓展名的可能是空占位，忽略
            let is_tree = f
                .get("Type")
                .map(|v| v.as_str() == Some("tree"))
                .unwrap_or(false);
            if is_tree {
                continue;
            }
            let rel = path[prefix.len()..].to_string();
            if rel.is_empty() || skip_repo_file(dir, path) {
                continue;
            }
            let size = f.get("Size").and_then(|v| v.as_u64()).unwrap_or(0);
            out.push(RepoFile { rel, size });
        }
    }
    if out.is_empty() {
        return Err(format!("模型 {dir} 的文件清单为空"));
    }
    Ok(out)
}

/// 下载一个 Sherpa 模型目录（递归多文件，带整体进度条与真实大小）。
#[tauri::command]
pub async fn sherpa_download_model(
    app: AppHandle,
    state: State<'_, SherpaModelsState>,
    model_id: String,
) -> Result<(), String> {
    let (dir, _name, _lang, _voice, _mtype) = sherpa_catalog_defs()
        .into_iter()
        .find(|(d, _, _, _, _)| d == &model_id)
        .ok_or_else(|| format!("未知的 Sherpa 模型: {model_id}"))?;

    {
        let mut in_flight = state.downloading.lock().await;
        if in_flight.contains(&model_id) {
            return Err(format!("模型 {model_id} 正在下载中"));
        }
        in_flight.insert(model_id.clone());
    }

    let result = run_download(&app, &state, &dir).await;
    {
        let mut in_flight = state.downloading.lock().await;
        in_flight.remove(&model_id);
    }
    result
}

/// 计算模型目录总大小（字节），供列表展示“模型大小”。
async fn repo_dir_total_bytes(dir: &str) -> u64 {
    let files = fetch_repo_files(dir).await;
    match files {
        Ok(files) => files.iter().map(|f| f.size).sum(),
        Err(_) => 0,
    }
}

async fn run_download(
    app: &AppHandle,
    state: &State<'_, SherpaModelsState>,
    dir: &str,
) -> Result<(), String> {
    let mgr = get_manager(state).await?;
    let model_root = mgr.model_dir();
    let target_dir = model_root.join(dir);
    let _ = tokio::fs::create_dir_all(&target_dir).await;

    let files = fetch_repo_files(dir).await?;
    let total: u64 = files.iter().map(|f| f.size).sum();
    if total == 0 {
        return Err(format!("模型 {dir} 无可下载文件（大小未知）"));
    }

    // espeak-ng-data 由多个模型共享，先下载一次再按需拷贝
    let shared_cache = model_root.join("_cache").join("espeak-ng-data");
    let shared_espeak = dir_has_espeak(&files);

    let client = crate::utils::download::build_download_client()
        .map_err(|e| format!("创建下载客户端失败: {e}"))?;

    let done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let concurrency = 6usize;
    let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let app_progress = app.clone();
    let mid = dir.to_string();
    let dir_owned = dir.to_string();
    let total_f = total as f64;

    let mut tasks = Vec::new();
    for f in files {
        let client = client.clone();
        let sem = sem.clone();
        let target_dir = target_dir.clone();
        let shared_cache = shared_cache.clone();
        let app_progress = app_progress.clone();
        let done_for_file = done.clone();
        let mid = mid.clone();
        let dir_owned = dir_owned.clone();

        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore");
            // espeak-ng-data：先从共享缓存下载；若已存在则直接指向目标目录
            let is_espeak = f.rel.starts_with("espeak-ng-data/");
            let sub = if is_espeak {
                f.rel["espeak-ng-data/".len()..].to_string()
            } else {
                String::new()
            };
            let dest = if is_espeak && shared_cache.join(&sub).exists() {
                target_dir.join("espeak-ng-data").join(&sub)
            } else if is_espeak {
                shared_cache.join(&sub)
            } else {
                target_dir.join(&f.rel)
            };
            if let Some(parent) = dest.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            let url = format!("{SHERPA_FILE_DL}/{dir_owned}/{}", f.rel.replace(' ', "%20"));
            let result =
                crate::utils::download::download_to_file(&client, &url, &dest, None, None, f.size)
                    .await;
            done_for_file.fetch_add(f.size, std::sync::atomic::Ordering::Relaxed);
            let done_now = done_for_file.load(std::sync::atomic::Ordering::Relaxed);
            let percent = (done_now as f64 / total_f * 100.0)
                .round()
                .clamp(0.0, 100.0);
            let _ = app_progress.emit(
                "tts://sherpa-download-progress",
                serde_json::json!({
                    "asset_id": mid,
                    "percent": percent,
                    "bytes_done": done_now,
                    "total_bytes": total as u64,
                }),
            );
            if let Err(e) = result {
                return Err(format!("下载 {} 失败: {e}", f.rel));
            }
            Ok::<(), String>(())
        }));
    }

    let mut any_err = None;
    for task in tasks {
        // `task.await` 是 `Result<Result<(), String>, JoinError>`：内层 `Err` 才是
        // 单文件下载失败，必须一并捕获，否则会被当成下载成功（留下半成品模型）。
        match task.await {
            Err(join_err) => {
                any_err.get_or_insert(format!("下载任务异常: {join_err}"));
                break;
            },
            Ok(Err(e)) => {
                any_err.get_or_insert(e);
            },
            Ok(Ok(())) => {},
        }
    }

    // 若已下过共享 espeak-ng-data，则拷贝进本模型目录（失败清理半成品并报错）
    if shared_espeak {
        let src = model_root.join("_cache").join("espeak-ng-data");
        let dst = target_dir.join("espeak-ng-data");
        if src.exists() {
            if let Err(e) = copy_dir_tree(&src, &dst).await {
                let _ = tokio::fs::remove_dir_all(&target_dir).await;
                return Err(e);
            }
        }
    }

    if let Some(e) = any_err {
        let _ = tokio::fs::remove_dir_all(&target_dir).await;
        return Err(e);
    }
    // 清理空 spec 目录项（modelscope 会有 dict/ 等空目录，无需特殊处理）

    let _ = mgr.initialize().await;
    Ok(())
}

fn dir_has_espeak(files: &[RepoFile]) -> bool {
    files.iter().any(|f| f.rel.starts_with("espeak-ng-data/"))
}

async fn copy_dir_tree(src: &std::path::Path, dst: &std::path::Path) -> Result<(), String> {
    tokio::fs::create_dir_all(dst)
        .await
        .map_err(|e| format!("mkdir {dst:?}: {e}"))?;
    let mut entries = tokio::fs::read_dir(src)
        .await
        .map_err(|e| format!("read_dir {src:?}: {e}"))?;
    let mut items = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        items.push(entry.path());
    }
    for path in items {
        let file_name = path.file_name().unwrap_or_default();
        let dest = dst.join(file_name);
        let is_dir = tokio::fs::metadata(&path)
            .await
            .map(|m| m.is_dir())
            .unwrap_or(false);
        if is_dir {
            Box::pin(copy_dir_tree(&path, &dest)).await?;
        } else {
            tokio::fs::copy(&path, &dest)
                .await
                .map_err(|e| format!("复制文件失败 {path:?} -> {dest:?}: {e}"))?;
        }
    }
    Ok(())
}

/// 试听合成：根据前端传入的 settings 构造 adapter 并合成一段 wav，返回临时路径。
#[derive(serde::Deserialize)]
pub struct SherpaPreviewSettings {
    pub sherpa_onnx_model_name: Option<String>,
    pub sherpa_onnx_model_type: Option<String>,
    pub sherpa_onnx_lang: Option<String>,
    pub sherpa_onnx_voice: Option<String>,
    pub sherpa_onnx_use_gpu: Option<bool>,
    pub sherpa_onnx_speed: Option<f32>,
    pub sherpa_onnx_ref_audio_path: Option<String>,
    pub sherpa_onnx_ref_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SherpaPreviewResult {
    pub success: bool,
    pub audio_path: String,
}

/// 试听合成命令（前端 `test_sherpa_onnx_voice`）。
#[tauri::command]
pub async fn test_sherpa_onnx_voice(
    app: AppHandle,
    text: String,
    settings: SherpaPreviewSettings,
) -> Result<Option<SherpaPreviewResult>, String> {
    let model_name = settings
        .sherpa_onnx_model_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| "未选择 Sherpa 模型".to_string())?;

    let model_root = crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root()
        .join(&model_name);

    let model_type = settings
        .sherpa_onnx_model_type
        .clone()
        .unwrap_or_else(|| "vits".to_string());
    let language = settings
        .sherpa_onnx_lang
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "zh".to_string());
    let voice = settings
        .sherpa_onnx_voice
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "female".to_string());
    let use_gpu = settings.sherpa_onnx_use_gpu.unwrap_or(false);

    let tts_config = crate::config::app_config::AppConfig::load(&app)
        .unwrap_or_default()
        .tts;

    let mut adapter = SherpaOnnxAdapter::new(
        tts_config,
        model_root.to_string_lossy().to_string(),
        model_type,
        language,
        voice,
        use_gpu,
    )
    .map_err(|e| e.to_string())?;

    // 参考音频（零样本克隆）：音频与文本必须成对，只有其一无法生效，明确报错
    // 以免用户误以为已启用克隆。
    let ref_audio = settings
        .sherpa_onnx_ref_audio_path
        .clone()
        .filter(|s| !s.trim().is_empty());
    let ref_text = settings
        .sherpa_onnx_ref_text
        .clone()
        .filter(|s| !s.trim().is_empty());
    match (&ref_audio, &ref_text) {
        (Some(ref_path), Some(ref_text)) => {
            let p = PathBuf::from(ref_path);
            if let Ok((samples, sr)) = load_ref_audio(&p) {
                adapter.set_reference_audio(samples, sr, ref_text.clone());
            }
        },
        (Some(_), None) => {
            return Err("已选择参考音频但未填写参考文本：零样本克隆需同时提供两者".to_string());
        },
        (None, Some(_)) => {
            return Err("已填写参考文本但未选择参考音频：零样本克隆需同时提供两者".to_string());
        },
        (None, None) => {},
    }
    if let Some(speed) = settings.sherpa_onnx_speed {
        adapter.set_speed(speed.clamp(0.5, 2.0));
    }

    let audio = adapter
        .generate_voice(&text, "")
        .await
        .map_err(|e| format!("Sherpa 合成失败: {e}"))?;

    let out_path = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("无法解析缓存目录: {e}"))?
        .join("tts")
        .join(format!(
            "sherpa_preview_{}.wav",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ));
    if let Some(parent) = out_path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    tokio::fs::write(&out_path, &audio)
        .await
        .map_err(|e| format!("写入试听音频失败: {e}"))?;

    Ok(Some(SherpaPreviewResult {
        success: true,
        audio_path: out_path.to_string_lossy().to_string(),
    }))
}

/// 打开 Sherpa 模型目录（文件管理器）。
///
/// Android 经 `storage-permission` 插件在系统文件管理器中打开目录；
/// 桌面端用系统默认方式（explorer / open / xdg-open）。
#[tauri::command]
pub async fn open_sherpa_onnx_model_manager(app: tauri::AppHandle) -> Result<(), String> {
    let dir = crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root();
    let _ = tokio::fs::create_dir_all(&dir).await;

    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let handle = &app.state::<crate::StoragePermissionPluginHandle>().0;
        // 必须以 JSON object 形式传参（Kotlin 侧通过 invoke.parseArgs 取 `path`），
        // 不能传裸字符串，否则 Android 插件无法解析。
        handle
            .run_mobile_plugin_async::<()>(
                "openModelDir",
                serde_json::json!({ "path": dir.to_string_lossy() }),
            )
            .await
            .map_err(|e| format!("打开模型文件夹失败: {e}"))?;
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        crate::utils::system::open_folder(&dir.to_string_lossy())
    }
}

/// Sherpa 模型目录的存储访问状态（移动端权限 + 模型根目录路径）。
#[derive(Debug, Serialize)]
pub struct SherpaStorageStatus {
    /// 是否已获得外部存储读写权限（桌面端恒为 `true`）。
    pub granted: bool,
    /// 模型根目录（模型文件夹 `<name>/` 直接位于该目录下）。
    pub model_root: String,
}

/// 检查 Sherpa-ONNX 模型目录的存储访问权限。
///
/// 模型目录统一位于应用数据目录（`data/sherpa_onnx_models`，Android 为应用专属
/// 沙箱 `Android/data/<package>/files/sherpa_onnx_models`），无需任何存储权限，
/// 因此恒返回已授权；同时返回模型根目录路径供前端展示。
#[tauri::command]
pub async fn check_sherpa_storage_permission(
    _app: tauri::AppHandle,
) -> Result<SherpaStorageStatus, String> {
    let model_root = crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root()
        .to_string_lossy()
        .to_string();
    Ok(SherpaStorageStatus {
        granted: true,
        model_root,
    })
}

/// 请求 Sherpa-ONNX 模型目录的存储访问权限。
///
/// 模型目录位于应用数据目录，无需存储权限，恒视为已授权（保留此命令仅为前端
/// 兼容，不再弹出任何系统授权流程）。
#[tauri::command]
pub async fn request_sherpa_storage_permission(
    _app: tauri::AppHandle,
) -> Result<SherpaStorageStatus, String> {
    let model_root = crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root()
        .to_string_lossy()
        .to_string();
    Ok(SherpaStorageStatus {
        granted: true,
        model_root,
    })
}
