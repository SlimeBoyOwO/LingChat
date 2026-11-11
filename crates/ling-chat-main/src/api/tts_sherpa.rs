//! Sherpa-ONNX 本地 TTS 模型管理：列表、下载、删除、试听合成。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex, RwLock};

use crate::ai_service::tts::adapters::sherpa_onnx::load_reference_audio as load_ref_audio;
use crate::ai_service::tts::adapters::sherpa_onnx::{
    SherpaOnnxAdapter, SherpaSpeaker, SherpaTuning,
};
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
    let entry = catalog_entry(&info.model_name);
    // 已装模型优先显示清单里的友好名。
    let mut display_name = entry
        .map(|e| e.display_name.to_string())
        .unwrap_or_else(|| info.model_name.clone());
    // 桌面专用模型在 Android 上会输出静音/垃圾样本（kokoro int8，上游 #3754）。
    if entry
        .is_some_and(|e| e.platforms == SherpaPlatforms::DesktopOnly && cfg!(target_os = "android"))
    {
        display_name.push_str("（当前平台不可用，建议删除）");
    }
    SherpaModelRecord {
        id: info.model_name.clone(),
        display_name,
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
        let id = rec.id.clone();
        if let Some(entry) = catalog_entry(&id) {
            size_tasks.push(async move { (id, repo_dir_total_bytes(entry).await) });
        }
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

/// 模型里可选的全部音色。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SherpaSpeakersInfo {
    pub num_speakers: usize,
    pub speakers: Vec<SherpaSpeaker>,
}

/// 列出模型自带的全部说话人（读 ONNX metadata，不加载模型）；多文件模型返回错误。
#[tauri::command]
pub async fn sherpa_list_speakers(model_name: String) -> Result<SherpaSpeakersInfo, String> {
    let name = model_name.trim().to_string();
    if name.is_empty() {
        return Err("未选择 Sherpa 模型".to_string());
    }
    // 模型名就是目录名；挡掉 `..` / 路径分隔符，避免拼出模型根目录以外的路径。
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("非法模型名: {name}"));
    }
    let dir =
        crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root().join(&name);
    if !dir.is_dir() {
        return Err(format!("模型目录不存在: {}", dir.display()));
    }

    // 顺序读一遍模型文件（114 MB 量级），别堵住 async 运行时。
    let speakers = tokio::task::spawn_blocking(move || {
        crate::ai_service::tts::adapters::sherpa_onnx::list_speakers(&dir)
    })
    .await
    .map_err(|e| format!("读取音色失败: {e}"))?
    .map_err(|e| e.to_string())?;

    Ok(SherpaSpeakersInfo {
        num_speakers: speakers.len(),
        speakers,
    })
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

// 内置模型默认走 modelscope `gomodels/sherpa` 镜像；Android 需要未量化的 kokoro
// （int8 在 arm64 上输出静音，上游 #3754），那份只在 HuggingFace 上有。

/// 模型文件的来源站点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SherpaSource {
    /// modelscope：接口按「仓库」列出所有文件，需按模型目录前缀过滤。
    ModelScope,
    /// HuggingFace：仓库即模型目录，直接取文件树；国内走 hf-mirror 镜像。
    HuggingFace,
}

/// 模型适用的平台。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SherpaPlatforms {
    All,
    /// 仅桌面（Windows/Linux/macOS）。
    DesktopOnly,
    /// 仅 Android。
    AndroidOnly,
}

impl SherpaPlatforms {
    fn matches_current(self) -> bool {
        match self {
            SherpaPlatforms::All => true,
            SherpaPlatforms::DesktopOnly => !cfg!(target_os = "android"),
            SherpaPlatforms::AndroidOnly => cfg!(target_os = "android"),
        }
    }
}

/// 内置可下载模型。
struct SherpaCatalogEntry {
    /// 模型目录名。同时是前端 `id`、磁盘目录名，以及适配器按目录反查类型的键。
    dir: &'static str,
    /// 下载仓库：modelscope / HuggingFace 都是 `owner/repo`。
    repo: &'static str,
    source: SherpaSource,
    display_name: &'static str,
    language: &'static str,
    voice: &'static str,
    /// 适配器加载该模型所需的 `model_type`。
    model_type: &'static str,
    platforms: SherpaPlatforms,
}

const MS_ENDPOINT: &str = "https://www.modelscope.cn";
/// HuggingFace 官方站国内访问不稳，统一走 hf-mirror 镜像。
const HF_ENDPOINT: &str = "https://hf-mirror.com";

const SHERPA_CATALOG: &[SherpaCatalogEntry] = &[
    SherpaCatalogEntry {
        dir: "kokoro-int8-multi-lang-v1_1",
        repo: "gomodels/sherpa",
        source: SherpaSource::ModelScope,
        display_name: "Kokoro 多语言 103 音色 (int8)",
        language: "zh",
        voice: "female",
        model_type: "kokoro",
        // int8 在 arm64 Android 上会输出静音或钉死的垃圾样本（上游 #3754），
        // 只在桌面端提供；Android 用下面的未量化版本。
        platforms: SherpaPlatforms::DesktopOnly,
    },
    SherpaCatalogEntry {
        dir: "kokoro-multi-lang-v1_1",
        repo: "csukuangfj/kokoro-multi-lang-v1_1",
        source: SherpaSource::HuggingFace,
        display_name: "Kokoro 多语言 103 音色 (未量化)",
        language: "zh",
        voice: "female",
        model_type: "kokoro",
        platforms: SherpaPlatforms::AndroidOnly,
    },
    SherpaCatalogEntry {
        dir: "matcha-icefall-zh-baker",
        repo: "gomodels/sherpa",
        source: SherpaSource::ModelScope,
        display_name: "Matcha 中文 (新汉语真人) · icefall",
        language: "zh",
        voice: "female",
        model_type: "matcha",
        platforms: SherpaPlatforms::All,
    },
    SherpaCatalogEntry {
        dir: "matcha_tts_zh_en_20251010",
        repo: "gomodels/sherpa",
        source: SherpaSource::ModelScope,
        display_name: "Matcha 中英 (MMTTS)",
        language: "zh",
        voice: "female",
        model_type: "matcha",
        platforms: SherpaPlatforms::All,
    },
    SherpaCatalogEntry {
        dir: "sherpa-onnx-zipvoice-distill-zh-en-emilia",
        repo: "gomodels/sherpa",
        source: SherpaSource::ModelScope,
        display_name: "ZipVoice 中英 (Emilia)",
        language: "zh",
        voice: "female",
        model_type: "zipvoice",
        platforms: SherpaPlatforms::All,
    },
    // 本地离线 ASR（SenseVoice）：用于微调数据集自动标注，与 TTS 音色共用
    // 该下载页。model_type 以 `asr_` 前缀区分，适配器与角色音色列表据此
    // 识别「这是识别模型，不是用于合成的音色」。
    SherpaCatalogEntry {
        dir: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17",
        repo: "gomodels/sherpa",
        source: SherpaSource::ModelScope,
        display_name: "SenseVoice 中英日韩粤 离线识别 (int8)",
        language: "zh",
        voice: "asr",
        model_type: "asr_sense_voice",
        platforms: SherpaPlatforms::All,
    },
];

/// 当前平台可下载的内置模型。
fn catalog_entries() -> impl Iterator<Item = &'static SherpaCatalogEntry> {
    SHERPA_CATALOG
        .iter()
        .filter(|entry| entry.platforms.matches_current())
}

/// 按目录名查清单条目（不过滤平台：已装模型需要能反查类型）。
fn catalog_entry(dir: &str) -> Option<&'static SherpaCatalogEntry> {
    SHERPA_CATALOG.iter().find(|entry| entry.dir == dir)
}

/// 内置清单里「模型目录名 → model_type」；用户导入的模型返回 `None`。
pub(crate) fn catalog_model_type(dir: &str) -> Option<&'static str> {
    catalog_entry(dir).map(|entry| entry.model_type)
}

/// 列出当前平台的内置模型（未带安装状态，由 `sherpa_list_models` 补全）。
pub fn sherpa_catalog() -> Vec<SherpaModelRecord> {
    catalog_entries()
        .map(|entry| SherpaModelRecord {
            id: entry.dir.to_string(),
            display_name: entry.display_name.to_string(),
            model_type: entry.model_type.to_string(),
            language: entry.language.to_string(),
            voice: entry.voice.to_string(),
            size_bytes: 0,
            installed: false,
            path: String::new(),
        })
        .collect()
}

/// 下载时跳过的冗余文件（pytorch 权重、示例音频等）。
fn skip_repo_file(dir: &str, rel: &str) -> bool {
    if rel.ends_with("pytorch_model.bin") || rel.ends_with("pytorch_model_sherpa.bin") {
        return true;
    }
    if rel.ends_with(".wav") || rel.ends_with(".gitattributes") {
        return true;
    }
    // ZipVoice：若存在非 int8 的 encoder/decoder，则跳过 int8 冗余
    if dir == "sherpa-onnx-zipvoice-distill-zh-en-emilia"
        && (rel.ends_with("fm_decoder_int8.onnx") || rel.ends_with("text_encoder_int8.onnx"))
    {
        return true;
    }
    // SenseVoice ASR：只用 int8 推理（尺寸小且精度够），跳过 937MB 的完整版。
    if dir == "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17"
        && (rel == "model.onnx"
            || rel == "model.fp32.onnx"
            || rel == "export-onnx.py"
            || rel.ends_with(".bpe.size"))
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

fn repo_files_api(entry: &SherpaCatalogEntry) -> String {
    match entry.source {
        SherpaSource::ModelScope => format!(
            "{MS_ENDPOINT}/api/v1/models/{}/repo/files?Revision=master&Recursive=true",
            entry.repo
        ),
        SherpaSource::HuggingFace => format!(
            "{HF_ENDPOINT}/api/models/{}/tree/main?recursive=true&blobs=true",
            entry.repo
        ),
    }
}

fn repo_file_url(entry: &SherpaCatalogEntry, rel: &str) -> String {
    let rel = rel.replace(' ', "%20");
    match entry.source {
        SherpaSource::ModelScope => format!(
            "{MS_ENDPOINT}/models/{}/resolve/master/{}/{rel}",
            entry.repo, entry.dir
        ),
        SherpaSource::HuggingFace => format!("{HF_ENDPOINT}/{}/resolve/main/{rel}", entry.repo),
    }
}

/// 拉取模型目录下的文件清单（过滤目录项与跳过项）。
async fn fetch_repo_files(entry: &SherpaCatalogEntry) -> Result<Vec<RepoFile>, String> {
    let client = crate::utils::download::build_download_client()
        .map_err(|e| format!("创建下载客户端失败: {e}"))?;
    let resp = client
        .get(repo_files_api(entry))
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

    let mut out = Vec::new();
    match entry.source {
        SherpaSource::ModelScope => {
            // modelscope 返回整个仓库的文件，需按 `<dir>/` 前缀过滤。
            let prefix = format!("{}/", entry.dir);
            if let Some(files) = value.pointer("/Data/Files").and_then(|f| f.as_array()) {
                for f in files {
                    let path = f.get("Path").and_then(|v| v.as_str()).unwrap_or_default();
                    if !path.starts_with(&prefix) {
                        continue;
                    }
                    // 目录项（Type=tree）跳过
                    let is_tree = f
                        .get("Type")
                        .map(|v| v.as_str() == Some("tree"))
                        .unwrap_or(false);
                    if is_tree {
                        continue;
                    }
                    let rel = path[prefix.len()..].to_string();
                    if rel.is_empty() || skip_repo_file(entry.dir, &rel) {
                        continue;
                    }
                    let size = f.get("Size").and_then(|v| v.as_u64()).unwrap_or(0);
                    out.push(RepoFile { rel, size });
                }
            }
        },
        SherpaSource::HuggingFace => {
            // HuggingFace 的 tree 接口直接返回该仓库（即模型目录）的文件数组。
            if let Some(files) = value.as_array() {
                for f in files {
                    if f.get("type").and_then(|v| v.as_str()) == Some("directory") {
                        continue;
                    }
                    let rel = f
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if rel.is_empty() || skip_repo_file(entry.dir, &rel) {
                        continue;
                    }
                    let size = f.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                    out.push(RepoFile { rel, size });
                }
            }
        },
    }

    if out.is_empty() {
        return Err(format!("模型 {} 的文件清单为空", entry.dir));
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
    let entry =
        catalog_entry(&model_id).ok_or_else(|| format!("未知的 Sherpa 模型: {model_id}"))?;
    // 清单里按平台分档（如 int8/未量化的 kokoro），挡住前端误传不适用于本平台的项。
    if !entry.platforms.matches_current() {
        return Err(format!("模型 {model_id} 不适用于当前平台"));
    }

    {
        let mut in_flight = state.downloading.lock().await;
        if in_flight.contains(&model_id) {
            return Err(format!("模型 {model_id} 正在下载中"));
        }
        in_flight.insert(model_id.clone());
    }

    let result = run_download(&app, &state, entry).await;
    {
        let mut in_flight = state.downloading.lock().await;
        in_flight.remove(&model_id);
    }
    result
}

/// 计算模型目录总大小（字节），供列表展示“模型大小”。
async fn repo_dir_total_bytes(entry: &SherpaCatalogEntry) -> u64 {
    match fetch_repo_files(entry).await {
        Ok(files) => files.iter().map(|f| f.size).sum(),
        Err(_) => 0,
    }
}

async fn run_download(
    app: &AppHandle,
    state: &State<'_, SherpaModelsState>,
    entry: &'static SherpaCatalogEntry,
) -> Result<(), String> {
    let dir = entry.dir;
    let mgr = get_manager(state).await?;
    let model_root = mgr.model_dir();
    let target_dir = model_root.join(dir);
    let _ = tokio::fs::create_dir_all(&target_dir).await;

    let files = fetch_repo_files(entry).await?;
    let total: u64 = files.iter().map(|f| f.size).sum();
    if total == 0 {
        return Err(format!("模型 {dir} 无可下载文件（大小未知）"));
    }

    // espeak-ng-data 由多个模型共享，先下载一次再按需拷贝。
    let shared_cache = model_root.join("_cache").join("espeak-ng-data");
    let shared_espeak = dir_has_espeak(&files);

    let client = crate::utils::download::build_download_client()
        .map_err(|e| format!("创建下载客户端失败: {e}"))?;

    let done = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let concurrency = 6usize;
    let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let app_progress = app.clone();
    let mid = dir.to_string();
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

        tasks.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.expect("semaphore");
            // espeak-ng-data 走共享缓存。
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
            let url = repo_file_url(entry, &f.rel);
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

    // 把共享 espeak-ng-data 拷进本模型目录；失败则清理半成品。
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

/// 试听合成：按前端 settings 构造 adapter，合成 WAV 并返回临时路径。
#[derive(serde::Deserialize)]
pub struct SherpaPreviewSettings {
    pub sherpa_onnx_model_name: Option<String>,
    pub sherpa_onnx_model_type: Option<String>,
    pub sherpa_onnx_lang: Option<String>,
    pub sherpa_onnx_voice: Option<String>,
    pub sherpa_onnx_use_gpu: Option<bool>,
    pub sherpa_onnx_speed: Option<f32>,
    pub sherpa_onnx_num_steps: Option<i32>,
    pub sherpa_onnx_guidance_scale: Option<f32>,
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
    // 试听文本里的假名/谚文优先于页面语言。
    let language = crate::ai_service::tts::text_language::detect(&text)
        .map(str::to_string)
        .or_else(|| {
            settings
                .sherpa_onnx_lang
                .clone()
                .filter(|s| !s.trim().is_empty())
        })
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
        SherpaTuning::from_options(
            settings.sherpa_onnx_num_steps,
            settings.sherpa_onnx_guidance_scale,
        ),
    )
    .map_err(|e| e.to_string())?;

    // 参考音频与文本必须成对，否则明确报错。
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
            // 加载失败必须报错：静默忽略会让用户以为克隆已启用，实际只是普通合成。
            let (samples, sr) = load_ref_audio(&p).map_err(|e| format!("参考音频加载失败: {e}"))?;
            adapter.set_reference_audio(samples, sr, ref_text.clone());
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

/// 参考音频导入结果（零样本声音克隆用）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SherpaRefAudioImport {
    /// 落盘后的本地路径，前端写回 `sherpa_onnx_ref_audio_path`。
    pub path: String,
    /// 展示用文件名。
    pub file_name: String,
}

/// 零样本克隆参考音频的落盘目录：`data/sherpa_onnx_ref_audio/`。
pub fn sherpa_onnx_ref_audio_root() -> PathBuf {
    crate::data_dir::get_data_dir().join("sherpa_onnx_ref_audio")
}

/// 导入零样本参考音频：复制进应用数据目录（Android 经 SAF 落盘，避免 content:// 读不了），
/// 并统一转成 16-bit PCM WAV。
#[tauri::command]
pub async fn import_sherpa_ref_audio(
    app: AppHandle,
    path: String,
) -> Result<SherpaRefAudioImport, String> {
    // Android 的 content:// 先落到 cache；桌面端原路径直通。
    let src =
        crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &path).await?;

    let result: Result<SherpaRefAudioImport, String> = async {
        // 落盘名取原文件名（清理非法字符），扩展名固定 wav。
        let stem =
            crate::ai_service::tts::local::saf_bridge::sanitize_staged_filename(&src.display_name)
                .and_then(|name| {
                    Path::new(&name)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.trim().to_string())
                })
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "ref_audio".to_string());

        let dir = sherpa_onnx_ref_audio_root();
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| format!("创建参考音频目录失败: {e}"))?;

        // 同名不覆盖：追加 `_2`/`_3`…（旧文件可能仍被其它角色引用）。
        let mut file_name = format!("{stem}.wav");
        let mut counter = 2u32;
        while dir.join(&file_name).exists() {
            if counter > 999 {
                file_name = format!(
                    "{stem}_{}.wav",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis())
                        .unwrap_or(0)
                );
                break;
            }
            file_name = format!("{stem}_{counter}.wav");
            counter += 1;
        }

        // sherpa 只认 WAV，格式问题在这里消化。
        let dest = dir.join(&file_name);
        crate::utils::audio::transcode_to_wav_16bit(&src.path, &dest).map_err(|e| {
            use crate::utils::audio::TranscodeError;
            match e {
                // 用户可自行解决的几种：给错误码，由前端翻译。
                TranscodeError::Unsupported(reason) => {
                    tracing::warn!("[tts_sherpa] 参考音频格式不支持: {reason}");
                    "SHERPA_REF_AUDIO_UNSUPPORTED".to_string()
                },
                TranscodeError::TooLong { seconds, .. } => {
                    tracing::warn!("[tts_sherpa] 参考音频过长: {seconds:.0} 秒");
                    "SHERPA_REF_AUDIO_TOO_LONG".to_string()
                },
                TranscodeError::OpusMultichannel(channels) => {
                    tracing::warn!(
                        "[tts_sherpa] 参考音频是多声道 Opus（{channels} 声道），暂不支持"
                    );
                    "SHERPA_REF_AUDIO_OPUS".to_string()
                },
                // 其余（IO、写盘）直接给中文说明。
                TranscodeError::Other(msg) => msg,
            }
        })?;

        tracing::info!(
            "[tts_sherpa] import_ref_audio: {} -> {}",
            src.path.display(),
            dest.display()
        );

        Ok(SherpaRefAudioImport {
            path: dest.to_string_lossy().to_string(),
            file_name,
        })
    }
    .await;

    // SAF 暂存文件用完即删（桌面端 cleanup_after_import 为 false，不动用户原文件）。
    if src.cleanup_after_import {
        let _ = tokio::fs::remove_file(&src.path).await;
    }

    result
}

/// 打开 Sherpa 模型目录（Android 经 `storage-permission`，桌面端用系统默认方式）。
#[tauri::command]
pub async fn open_sherpa_onnx_model_manager(app: tauri::AppHandle) -> Result<(), String> {
    let dir = crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root();
    let _ = tokio::fs::create_dir_all(&dir).await;

    // Android 无对应系统入口（`open_folder` 直接返回 Ok）。
    let _ = app;
    crate::utils::system::open_folder(&dir.to_string_lossy())
}

/// Sherpa 模型目录的存储访问状态（移动端权限 + 模型根目录路径）。
#[derive(Debug, Serialize)]
pub struct SherpaStorageStatus {
    /// 是否已获得外部存储读写权限（桌面端恒为 `true`）。
    pub granted: bool,
    /// 模型根目录（模型文件夹 `<name>/` 直接位于该目录下）。
    pub model_root: String,
}

/// 模型目录位于应用数据目录，无需存储权限，恒返回已授权。
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

/// 恒视为已授权；保留此命令仅为前端兼容。
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
