//! IndexTTS2 进程内嵌引擎（PyO3）。
//!
//! 与服务器方案不同：直接把引擎的嵌入式 Python（`engine/runtime/python310.dll`）
//! 加载进主程序进程，模型常驻内存，合成请求经专用引擎线程调用 Python 推理
//! 函数完成——无端口、无 HTTP、无独立进程。
//!
//! 目录布局（全部相对可移植）：
//! - `<exe>/engine/`：代码 + 运行环境（runtime/、repo/；repo 随 exe
//!   内嵌资源在首次运行时释放，控制面 Python 文件不再落盘）
//! - `<data>/third_party/IndexTTS-AMD/`：模型数据与运行时产物
//!   （checkpoints/、voices/、outputs/、缓存与日志）
//!
//! 结构：
//! - 引擎线程（唯一）：持有解释器与模型；环境变量 / 解释器初始化仅一次
//! - 通道：所有合成请求排队进入引擎线程（GPU 串行，与服务器 `_gpu_lock` 一致）
//! - `IndexTtsEmbeddedAdapter`：实现 `TtsAdapter`，对上层与 HTTP 适配器无差别

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyTuple};
use serde::Serialize;
use serde_json::{json, Value as JsonValue};
use tokio::sync::oneshot;

use super::audio::{encode_wav_pcm16, StreamingWavWriter};
use super::emotion::{build_plan, vector_from_slice, EmotionCache, EmotionPlan, EmotionVector};
use crate::ai_service::tts::provider::TtsAdapter;
use crate::config::tts::TtsConfig;
use crate::init::static_copy;

/// Python shim 源码（随主程序编译进 exe，不落盘）
const SHIM_SOURCE: &str = include_str!("embedded_engine.py");
/// repo/indextts 包（zip，内嵌，首启动释放到 engine/repo/）
const REPO_ZIP: &[u8] = include_bytes!("../../../assets/repo.zip");

const DEFAULT_DATA_DIR_NAME: &str = "IndexTTS-AMD";
/// 单个合成请求的最长等待（含排队与首次模型加载）
const SYNTH_TIMEOUT: Duration = Duration::from_secs(600);

enum SynthTarget {
    Memory,
    File(PathBuf),
}

enum SynthOutput {
    Bytes(Vec<u8>),
    FileWritten,
}

struct SynthJob {
    text: String,
    emotion: EmotionPlan,
    voice_path: PathBuf,
    target: SynthTarget,
    cancelled: Arc<AtomicBool>,
    reply: oneshot::Sender<Result<SynthOutput, String>>,
}

static ENGINE_TX: OnceLock<mpsc::Sender<SynthJob>> = OnceLock::new();

fn engine_start_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: OnceLock<std::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

// ========== 引擎就绪状态（前端启动门查询/等待） ==========

/// 引擎状态：NotUsed（未使用内置）/ Loading（加载中）/ Ready（就绪）/ Failed（失败）
#[derive(Debug, Clone)]
enum EngineStatus {
    NotUsed,
    Loading,
    Ready,
    Failed(String),
}

fn status_cell() -> &'static (std::sync::Mutex<EngineStatus>, tokio::sync::Notify) {
    static CELL: OnceLock<(std::sync::Mutex<EngineStatus>, tokio::sync::Notify)> = OnceLock::new();
    CELL.get_or_init(|| {
        (
            std::sync::Mutex::new(EngineStatus::NotUsed),
            tokio::sync::Notify::new(),
        )
    })
}

fn set_status(status: EngineStatus) {
    let (lock, notify) = status_cell();
    *lock.lock().unwrap() = status;
    notify.notify_waiters();
}

/// 当前状态：(state, detail)；state ∈ not_used / loading / ready / failed
pub fn current_status() -> (String, String) {
    let (lock, _) = status_cell();
    match &*lock.lock().unwrap() {
        EngineStatus::NotUsed => ("not_used".into(), String::new()),
        EngineStatus::Loading => ("loading".into(), String::new()),
        EngineStatus::Ready => ("ready".into(), String::new()),
        EngineStatus::Failed(m) => ("failed".into(), m.clone()),
    }
}

/// 等待引擎就绪：not_used / ready 立即返回；failed 返回错误；loading 最多等 300 秒
pub async fn await_ready() -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_secs(300);
    loop {
        let (state, detail) = current_status();
        match state.as_str() {
            "not_used" | "ready" => return Ok(()),
            "failed" => {
                return Err(if detail.is_empty() {
                    "内嵌语音引擎初始化失败".into()
                } else {
                    detail
                })
            }
            _ => {}
        }
        if std::time::Instant::now() >= deadline {
            return Err("等待内嵌语音引擎就绪超时（300 秒）".into());
        }
        let notified = status_cell().1.notified();
        let _ = tokio::time::timeout(Duration::from_secs(2), notified).await;
    }
}

// ========== TTS 适配器 ==========

/// TTS 适配器：进程内嵌 IndexTTS2（`indextts2_builtin` 类型使用）。
#[derive(Debug, Clone)]
pub struct IndexTtsEmbeddedAdapter {
    speaker_id: i64,
    lang: String,
    emo_mode: String,
    data_dir: PathBuf,
}

impl IndexTtsEmbeddedAdapter {
    pub fn new(tts_config: &TtsConfig, speaker_id: i64, lang: impl Into<String>) -> Result<Self> {
        ensure_engine_started(tts_config)?;
        Ok(Self {
            speaker_id: speaker_id.max(0),
            lang: lang.into(),
            emo_mode: tts_config.indextts_emo_mode.clone(),
            data_dir: engine_data_dir(&tts_config.indextts_engine_dir),
        })
    }

    fn voice_path(&self) -> Result<PathBuf> {
        let voices_dir = self.data_dir.join("voices");
        let mut entries = std::fs::read_dir(&voices_dir)?
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| VOICE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
                        .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        });
        if entries.is_empty() {
            return Err(anyhow!("IndexTTS2 音色目录中没有可用文件"));
        }
        Ok(entries
            .get(self.speaker_id as usize)
            .cloned()
            .unwrap_or_else(|| entries[0].clone()))
    }

    async fn submit(&self, text: &str, emo: &str, target: SynthTarget) -> Result<SynthOutput> {
        let tx = ENGINE_TX
            .get()
            .ok_or_else(|| anyhow!("内嵌 IndexTTS2 引擎未初始化"))?
            .clone();
        let (reply_tx, reply_rx) = oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        tx.send(SynthJob {
            text: text.to_string(),
            emotion: build_plan(&self.emo_mode, emo, text),
            voice_path: self.voice_path()?,
            target,
            cancelled: cancelled.clone(),
            reply: reply_tx,
        })
        .map_err(|_| anyhow!("内嵌 IndexTTS2 引擎线程已停止"))?;
        match tokio::time::timeout(SYNTH_TIMEOUT, reply_rx).await {
            Ok(Ok(Ok(output))) => Ok(output),
            Ok(Ok(Err(error))) => Err(anyhow!("IndexTTS2 合成失败: {error}")),
            Ok(Err(_)) => Err(anyhow!("内嵌 IndexTTS2 引擎线程异常终止")),
            Err(_) => {
                cancelled.store(true, Ordering::Release);
                Err(anyhow!(
                    "IndexTTS2 合成超时（{} 秒），已请求取消",
                    SYNTH_TIMEOUT.as_secs()
                ))
            }
        }
    }
}

#[async_trait]
impl TtsAdapter for IndexTtsEmbeddedAdapter {
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>> {
        match self.submit(text, emo, SynthTarget::Memory).await? {
            SynthOutput::Bytes(bytes) => Ok(bytes),
            SynthOutput::FileWritten => Err(anyhow!("IndexTTS2 返回了错误的文件输出类型")),
        }
    }

    async fn generate_voice_to_file(&self, text: &str, emo: &str, file_path: &Path) -> Result<()> {
        match self
            .submit(text, emo, SynthTarget::File(file_path.to_path_buf()))
            .await?
        {
            SynthOutput::FileWritten => Ok(()),
            SynthOutput::Bytes(_) => Err(anyhow!("IndexTTS2 返回了错误的内存输出类型")),
        }
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("mode".into(), json!("embedded"));
        m.insert("speaker_id".into(), json!(self.speaker_id));
        m.insert("lang".into(), json!(self.lang));
        m.insert("emo_mode".into(), json!(self.emo_mode));
        m
    }
}

// ========== 目录解析 ==========

/// 代码目录：exe 旁边的 `engine/`（data_dir 的上一级即 exe 目录）
fn engine_code_dir() -> PathBuf {
    static_copy::get_data_dir()
        .parent()
        .map(|p| p.join("engine"))
        .unwrap_or_else(|| PathBuf::from("engine"))
}

/// 数据目录：模型/音色/产物（默认 `<data>/third_party/IndexTTS-AMD`，可被配置覆盖）
fn engine_data_dir(override_dir: &str) -> PathBuf {
    if !override_dir.trim().is_empty() {
        return PathBuf::from(override_dir.trim());
    }
    static_copy::get_data_dir()
        .join("third_party")
        .join(DEFAULT_DATA_DIR_NAME)
}

const VOICE_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg"];
const MAX_VOICE_FILE_BYTES: usize = 50 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct IndexTtsVoicePreset {
    pub id: usize,
    pub file_name: String,
    pub size: u64,
}

fn voice_extension(file_name: &str) -> Option<String> {
    Path::new(file_name)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .filter(|ext| VOICE_EXTENSIONS.contains(&ext.as_str()))
}

fn validate_voice_file_name(file_name: &str) -> Result<&str, String> {
    let trimmed = file_name.trim();
    if trimmed.is_empty()
        || Path::new(trimmed)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(trimmed)
        || voice_extension(trimmed).is_none()
    {
        return Err("音色文件名无效，仅支持 wav/mp3/flac/ogg 文件".into());
    }
    Ok(trimmed)
}

fn validate_voice_bytes(extension: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.is_empty() {
        return Err("音色文件为空".into());
    }
    if bytes.len() > MAX_VOICE_FILE_BYTES {
        return Err("音色文件不能超过 50 MB".into());
    }
    let valid_magic = match extension {
        "wav" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "flac" => bytes.starts_with(b"fLaC"),
        "ogg" => bytes.starts_with(b"OggS"),
        "mp3" => {
            bytes.starts_with(b"ID3")
                || (bytes.len() >= 2 && bytes[0] == 0xff && (bytes[1] & 0xe0) == 0xe0)
        }
        _ => false,
    };
    if !valid_magic {
        return Err(format!("文件内容不是有效的 {extension} 音频"));
    }
    Ok(())
}

pub fn list_voice_presets(tts_config: &TtsConfig) -> Result<Vec<IndexTtsVoicePreset>, String> {
    let voices_dir = engine_data_dir(&tts_config.indextts_engine_dir).join("voices");
    std::fs::create_dir_all(&voices_dir)
        .map_err(|e| format!("创建音色目录失败（{}）: {e}", voices_dir.display()))?;
    let mut entries = std::fs::read_dir(&voices_dir)
        .map_err(|e| format!("读取音色目录失败（{}）: {e}", voices_dir.display()))?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let file_name = path.file_name()?.to_str()?.to_string();
            voice_extension(&file_name)?;
            let size = entry.metadata().ok()?.len();
            Some((file_name, size))
        })
        .collect::<Vec<_>>();
    entries.sort_by_key(|(file_name, _)| file_name.to_lowercase());
    Ok(entries
        .into_iter()
        .enumerate()
        .map(|(id, (file_name, size))| IndexTtsVoicePreset {
            id,
            file_name,
            size,
        })
        .collect())
}

pub fn upload_voice_preset(
    tts_config: &TtsConfig,
    file_name: &str,
    file_data: &[u8],
) -> Result<Vec<IndexTtsVoicePreset>, String> {
    let safe_name = validate_voice_file_name(file_name)?;
    let extension = voice_extension(safe_name).ok_or_else(|| "不支持的音色文件格式".to_string())?;
    validate_voice_bytes(&extension, file_data)?;

    let voices_dir = engine_data_dir(&tts_config.indextts_engine_dir).join("voices");
    std::fs::create_dir_all(&voices_dir)
        .map_err(|e| format!("创建音色目录失败（{}）: {e}", voices_dir.display()))?;
    let target = voices_dir.join(safe_name);
    if target.exists() {
        return Err(format!("音色文件 {safe_name} 已存在，请先删除或更改文件名"));
    }
    let temp = voices_dir.join(format!(".upload-{}.tmp", uuid::Uuid::new_v4()));
    std::fs::write(&temp, file_data).map_err(|e| format!("写入临时音色文件失败: {e}"))?;
    if let Err(e) = std::fs::rename(&temp, &target) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("保存音色文件失败: {e}"));
    }
    tracing::info!("已导入 IndexTTS 音色: {}", target.display());
    list_voice_presets(tts_config)
}

pub fn delete_voice_preset(
    tts_config: &TtsConfig,
    file_name: &str,
) -> Result<Vec<IndexTtsVoicePreset>, String> {
    let safe_name = validate_voice_file_name(file_name)?;
    let presets = list_voice_presets(tts_config)?;
    if presets.len() <= 1 {
        return Err("至少需要保留一个音色预设，无法删除最后一个音色".into());
    }
    if !presets.iter().any(|preset| preset.file_name == safe_name) {
        return Err(format!("音色文件不存在: {safe_name}"));
    }
    let target = engine_data_dir(&tts_config.indextts_engine_dir)
        .join("voices")
        .join(safe_name);
    std::fs::remove_file(&target)
        .map_err(|e| format!("删除音色失败（{}）: {e}", target.display()))?;
    tracing::info!("已删除 IndexTTS 音色: {}", target.display());
    list_voice_presets(tts_config)
}

fn contains_voice_preset(voices_dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(voices_dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.path().is_file()
            && entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| VOICE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
                .unwrap_or(false)
    })
}

fn require_nonempty_file(path: &Path, missing: &mut Vec<String>) {
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {}
        _ => missing.push(path.display().to_string()),
    }
}

fn require_nonempty_dir(path: &Path, missing: &mut Vec<String>) {
    let valid = std::fs::read_dir(path)
        .ok()
        .and_then(|mut entries| entries.next())
        .is_some();
    if !valid {
        missing.push(path.display().to_string());
    }
}

/// Rust 在初始化 Python 前完整校验运行时和模型，避免等到深层 import 才报模糊错误。
fn verify_engine_resources(runtime_dir: &Path, data_dir: &Path) -> Result<()> {
    let mut missing = Vec::new();
    for path in [
        runtime_dir.join("python310.dll"),
        runtime_dir.join("python310.zip"),
        runtime_dir
            .join("Lib")
            .join("site-packages")
            .join("torch")
            .join("__init__.py"),
    ] {
        require_nonempty_file(&path, &mut missing);
    }
    require_nonempty_dir(
        &runtime_dir
            .join("Lib")
            .join("site-packages")
            .join("torch")
            .join("lib"),
        &mut missing,
    );

    let checkpoints = data_dir.join("checkpoints");
    let config_path = checkpoints.join("config.yaml");
    require_nonempty_file(&config_path, &mut missing);
    if let Ok(config_bytes) = std::fs::read(&config_path) {
        if let Ok(config) = serde_yaml::from_slice::<serde_yaml::Value>(&config_bytes) {
            let top_level = [
                "gpt_checkpoint",
                "w2v_stat",
                "s2mel_checkpoint",
                "emo_matrix",
                "spk_matrix",
            ];
            for key in top_level {
                if let Some(relative) = config.get(key).and_then(serde_yaml::Value::as_str) {
                    require_nonempty_file(&checkpoints.join(relative.trim()), &mut missing);
                } else {
                    missing.push(format!("{} 中缺少字段 {key}", config_path.display()));
                }
            }
            if let Some(relative) = config
                .get("dataset")
                .and_then(|value| value.get("bpe_model"))
                .and_then(serde_yaml::Value::as_str)
            {
                require_nonempty_file(&checkpoints.join(relative.trim()), &mut missing);
            } else {
                missing.push(format!(
                    "{} 中缺少字段 dataset.bpe_model",
                    config_path.display()
                ));
            }
            if let Some(relative) = config
                .get("qwen_emo_path")
                .and_then(serde_yaml::Value::as_str)
            {
                require_nonempty_dir(&checkpoints.join(relative.trim()), &mut missing);
            } else {
                missing.push(format!(
                    "{} 中缺少字段 qwen_emo_path",
                    config_path.display()
                ));
            }
        } else {
            missing.push(format!("{} 不是有效 YAML", config_path.display()));
        }
    }

    let hf_cache = checkpoints.join("hf_cache");
    for path in [
        hf_cache.join("semantic_codec_model.safetensors"),
        hf_cache.join("campplus_cn_common.bin"),
        hf_cache.join("bigvgan").join("config.json"),
        hf_cache.join("bigvgan").join("bigvgan_generator.pt"),
    ] {
        require_nonempty_file(&path, &mut missing);
    }
    require_nonempty_dir(&hf_cache.join("w2v-bert-2.0"), &mut missing);

    if missing.is_empty() {
        tracing::info!("IndexTTS2 Rust 资源校验通过");
        Ok(())
    } else {
        Err(anyhow!(
            "内置 IndexTTS2 资源不完整，共缺失 {} 项:\n{}",
            missing.len(),
            missing.join("\n")
        ))
    }
}

/// 根据内嵌资源内容生成版本戳，避免人工版本号漏改后继续使用旧 Python 代码。
fn bundled_version() -> u64 {
    fn update(mut hash: u64, bytes: &[u8]) -> u64 {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    let hash = update(0xcbf29ce484222325, SHIM_SOURCE.as_bytes());
    update(hash, REPO_ZIP)
}

/// 首启动只释放 repo/indextts 模型代码；控制面桥直接编译在 exe 中。
/// 版本戳由实际资源内容计算；关键文件缺失时即使戳存在也会重新释放。
fn ensure_extracted(code_dir: &Path) -> Result<()> {
    let stamp = code_dir.join(format!(".bundled-{:016x}", bundled_version()));
    let infer_path = code_dir.join("repo").join("indextts").join("infer_v2.py");
    if stamp.exists() && infer_path.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(code_dir)?;
    let repo_dir = code_dir.join("repo");
    let cursor = std::io::Cursor::new(REPO_ZIP);
    zip::ZipArchive::new(cursor)?.extract(&repo_dir)?;
    std::fs::write(&stamp, b"")?;
    if let Ok(entries) = std::fs::read_dir(code_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(".bundled-") && entry.path() != stamp {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    tracing::info!("内嵌引擎代码已释放到 {}", code_dir.display());
    Ok(())
}

// ========== 引擎启动 ==========

/// 启动内嵌引擎（幂等）：解析目录 → 释放代码资源 → 拉起引擎线程（线程内完成模型加载）。
pub fn ensure_engine_started(tts_config: &TtsConfig) -> Result<()> {
    let _start_guard = engine_start_lock()
        .lock()
        .map_err(|_| anyhow!("内嵌引擎启动锁已损坏"))?;
    if ENGINE_TX.get().is_some() {
        return Ok(());
    }
    let code_dir = engine_code_dir();
    let data_dir = engine_data_dir(&tts_config.indextts_engine_dir);
    let runtime_dir = code_dir.join("runtime");
    if let Err(error) = verify_engine_resources(&runtime_dir, &data_dir) {
        let msg = error.to_string();
        set_status(EngineStatus::Failed(msg.clone()));
        return Err(anyhow!(msg));
    }
    if let Err(e) = ensure_extracted(&code_dir) {
        let msg = format!("内嵌引擎代码释放失败: {e}");
        set_status(EngineStatus::Failed(msg.clone()));
        return Err(anyhow!(msg));
    }
    let voices_dir = data_dir.join("voices");
    for (label, required) in [
        ("模型配置", data_dir.join("checkpoints").join("config.yaml")),
        ("音色目录", voices_dir.clone()),
    ] {
        if !required.exists() {
            let msg = format!("{label}缺失（期望 {} 存在）", required.display());
            set_status(EngineStatus::Failed(msg.clone()));
            return Err(anyhow!(msg));
        }
    }
    if !contains_voice_preset(&voices_dir) {
        let msg = format!(
            "音色目录中没有可用预设（请将 wav/mp3/flac/ogg 放入 {}）",
            voices_dir.display()
        );
        set_status(EngineStatus::Failed(msg.clone()));
        return Err(anyhow!(msg));
    }
    let (tx, rx) = mpsc::channel::<SynthJob>();
    let emo_mode = tts_config.indextts_emo_mode.clone();
    set_status(EngineStatus::Loading);
    if let Err(e) = std::thread::Builder::new()
        .name("indextts-embed".into())
        .spawn(move || engine_thread_main(rx, code_dir, runtime_dir, data_dir, emo_mode))
    {
        let msg = format!("无法创建内嵌引擎线程: {e}");
        set_status(EngineStatus::Failed(msg.clone()));
        return Err(anyhow!(msg));
    }
    ENGINE_TX
        .set(tx)
        .map_err(|_| anyhow!("内嵌引擎发送通道重复初始化"))?;
    tracing::info!("内嵌 IndexTTS2 引擎线程已启动，模型加载中…");
    Ok(())
}

/// 引擎线程主函数：初始化 Python + 模型，然后串行处理合成请求。
fn engine_thread_main(
    rx: mpsc::Receiver<SynthJob>,
    code_dir: PathBuf,
    runtime_dir: PathBuf,
    data_dir: PathBuf,
    emo_mode: String,
) {
    match init_python(&code_dir, &runtime_dir, &data_dir, &emo_mode) {
        Ok(module) => {
            let mut emotion_cache = EmotionCache::load(data_dir.join("qwen_emo_cache.rust.json"));
            set_status(EngineStatus::Ready);
            tracing::info!("内嵌 IndexTTS2 引擎就绪");
            for job in rx.iter() {
                if job.reply.is_closed() {
                    tracing::debug!("跳过已取消的 IndexTTS2 合成请求");
                    continue;
                }
                let result = synth_blocking(&module, &job, &mut emotion_cache);
                let _ = job.reply.send(result);
            }
        }
        Err(e) => {
            set_status(EngineStatus::Failed(e.to_string()));
            tracing::error!("内嵌 IndexTTS2 引擎初始化失败: {e}");
            // 初始化失败：持续应答错误，避免调用方永久等待
            let err = e.to_string();
            for job in rx.iter() {
                let _ = job.reply.send(Err(err.clone()));
            }
        }
    }
}

fn resolve_emotion_vector(
    module: &Bound<'_, PyModule>,
    plan: &EmotionPlan,
    cache: &mut EmotionCache,
) -> Result<Option<EmotionVector>, String> {
    match plan {
        EmotionPlan::Vector(vector) => Ok(Some(*vector)),
        EmotionPlan::FollowReference => Ok(None),
        EmotionPlan::Analyze { cache_key, prompt } => {
            if let Some(vector) = cache.get(cache_key) {
                tracing::debug!("IndexTTS2 Qwen 情绪缓存命中: {cache_key}");
                return Ok(Some(vector));
            }
            let values: Vec<f32> = module
                .getattr("analyze_emotion")
                .map_err(|error| format!("shim.analyze_emotion 缺失: {error}"))?
                .call1((prompt.as_str(),))
                .and_then(|value| value.extract())
                .map_err(|error| format!("Qwen 情绪分析失败: {error}"))?;
            let vector = vector_from_slice(&values)?;
            if let Err(error) = cache.insert(cache_key.clone(), vector) {
                tracing::warn!("IndexTTS2 Qwen 情绪缓存保存失败: {error}");
            } else {
                tracing::info!("IndexTTS2 Qwen 情绪向量已由 Rust 缓存: {cache_key}");
            }
            Ok(Some(vector))
        }
    }
}

enum PcmSink {
    Memory {
        sample_rate: Option<u32>,
        pcm: Vec<u8>,
    },
    File(StreamingWavWriter),
}

impl PcmSink {
    fn push(&mut self, sample_rate: u32, pcm_chunk: &[u8]) -> Result<(), String> {
        match self {
            Self::Memory {
                sample_rate: current,
                pcm,
            } => {
                if let Some(current) = current {
                    if *current != sample_rate {
                        return Err(format!("PCM 流采样率发生变化: {current} -> {sample_rate}"));
                    }
                } else {
                    *current = Some(sample_rate);
                }
                pcm.extend_from_slice(pcm_chunk);
                Ok(())
            }
            Self::File(writer) => writer.push(sample_rate, pcm_chunk),
        }
    }

    fn finish(self) -> Result<SynthOutput, String> {
        match self {
            Self::Memory { sample_rate, pcm } => {
                let sample_rate = sample_rate.ok_or_else(|| "没有收到 PCM 分块".to_string())?;
                encode_wav_pcm16(sample_rate, &pcm).map(SynthOutput::Bytes)
            }
            Self::File(writer) => {
                writer.finish()?;
                Ok(SynthOutput::FileWritten)
            }
        }
    }
}

/// 同步执行一次流式合成（只在引擎线程内调用）。
fn synth_blocking(
    module: &Py<PyModule>,
    job: &SynthJob,
    emotion_cache: &mut EmotionCache,
) -> Result<SynthOutput, String> {
    Python::with_gil(|py| {
        let module = module.bind(py);
        let emotion_vector = resolve_emotion_vector(&module, &job.emotion, emotion_cache)?;
        let generator = module
            .getattr("synth_stream")
            .map_err(|error| format!("shim.synth_stream 缺失: {error}"))?
            .call1((
                job.text.as_str(),
                job.voice_path.to_string_lossy().to_string(),
                emotion_vector.map(|vector| vector.to_vec()),
                0.6f64,
                120i64,
            ))
            .map_err(|error| format!("启动 Python 流式推理失败: {error}"))?;
        let iterator = generator
            .iter()
            .map_err(|error| format!("Python 推理结果不是迭代器: {error}"))?;
        let mut sink = match &job.target {
            SynthTarget::Memory => PcmSink::Memory {
                sample_rate: None,
                pcm: Vec::new(),
            },
            SynthTarget::File(path) => PcmSink::File(StreamingWavWriter::create(path)?),
        };
        let mut chunks = 0usize;
        for item in iterator {
            if job.cancelled.load(Ordering::Acquire) || job.reply.is_closed() {
                tracing::info!("IndexTTS2 正在执行的流式请求已取消");
                return Err("合成请求已取消".into());
            }
            let item = item.map_err(|error| format!("读取 Python PCM 分块失败: {error}"))?;
            let tuple = item
                .downcast::<PyTuple>()
                .map_err(|_| "Python PCM 分块不是 (sample_rate, bytes)".to_string())?;
            if tuple.len() != 2 {
                return Err("Python PCM 分块元组长度不是 2".into());
            }
            let sample_rate_item = tuple
                .get_item(0)
                .map_err(|error| format!("读取 PCM 采样率失败: {error}"))?;
            let sample_rate: u32 = sample_rate_item
                .extract()
                .map_err(|error| format!("解析 PCM 采样率失败: {error}"))?;
            let pcm_item = tuple
                .get_item(1)
                .map_err(|error| format!("读取 PCM 字节失败: {error}"))?;
            let pcm = pcm_item
                .downcast::<PyBytes>()
                .map_err(|_| "Python PCM 分块不是 bytes".to_string())?;
            sink.push(sample_rate, pcm.as_bytes())?;
            chunks += 1;
        }
        tracing::debug!("IndexTTS2 Rust 流式接收完成，共 {chunks} 个 PCM 分块");
        sink.finish()
    })
}

/// 加载解释器 → 加载最小 shim（直接 import IndexTTS2 → 加载模型）。
fn init_python(
    code_dir: &Path,
    runtime_dir: &Path,
    data_dir: &Path,
    emo_mode: &str,
) -> Result<Py<PyModule>> {
    // 1) 环境变量（供 server_indextts 读取；进程级，必须在解释器初始化前设置）
    let miopen_db = data_dir.join("miopen").join("db_infer");
    let miopen_cache = data_dir.join("miopen").join("cache_infer");
    std::fs::create_dir_all(&miopen_db).ok();
    std::fs::create_dir_all(&miopen_cache).ok();
    let envs: Vec<(&str, String)> = vec![
        (
            "HF_HOME",
            data_dir.join(".hf-cache").to_string_lossy().into(),
        ),
        (
            "MODELSCOPE_CACHE",
            data_dir.join(".ms-cache").to_string_lossy().into(),
        ),
        ("INDEXTTS_DATA_DIR", data_dir.to_string_lossy().into()),
        ("INDEXTTS_RUNTIME_DIR", runtime_dir.to_string_lossy().into()),
        ("MIOPEN_FIND_MODE", "2".into()),
        ("MIOPEN_USER_DB_PATH", miopen_db.to_string_lossy().into()),
        (
            "MIOPEN_CUSTOM_CACHE_DIR",
            miopen_cache.to_string_lossy().into(),
        ),
        ("MIOPEN_ENABLE_LOGGING", "0".into()),
        ("MIOPEN_ENABLE_LOGGING_CMD", "0".into()),
        ("MIOPEN_LOG_LEVEL", "1".into()),
        ("INDEXTTS_EMO_MODE", emo_mode.to_string()),
        ("INDEXTTS_FP16", "1".into()),
        ("INDEXTTS_VOCODER_FP16", "1".into()),
        ("INDEXTTS_NUM_BEAMS", "1".into()),
        ("INDEXTTS_DIFFUSION_STEPS", "16".into()),
    ];
    for (k, v) in envs {
        std::env::set_var(k, v);
    }

    // 2) python310.dll 已随包置于 exe 旁边，进程启动时即被加载器载入。
    //    以"隔离配置"初始化解释器：显式模块搜索路径，
    //    不吃 PYTHONHOME / 注册表 / ._pth，彻底可移植（本线程后续经 with_gil 复用）
    unsafe { initialize_isolated_python(runtime_dir, code_dir)? };

    // 3) 加载最小 shim（直接 import IndexTTS2 → 加载模型，需数十秒）
    Python::with_gil(|py| -> Result<Py<PyModule>> {
        let sys = py
            .import_bound("sys")
            .map_err(|e| anyhow!("导入 sys 失败: {e}"))?;
        let path = sys
            .getattr("path")
            .map_err(|e| anyhow!("获取 sys.path 失败: {e}"))?;
        for p in [
            code_dir.to_string_lossy().to_string(),
            runtime_dir
                .join("Lib")
                .join("site-packages")
                .to_string_lossy()
                .to_string(),
        ] {
            path.call_method1("insert", (0, p))
                .map_err(|e| anyhow!("sys.path 注入失败: {e}"))?;
        }
        let module =
            PyModule::from_code_bound(py, SHIM_SOURCE, "embedded_engine.py", "embedded_engine")
                .map_err(|e| anyhow!("shim 编译失败: {e}"))?;
        let info = module
            .getattr("init")
            .map_err(|e| anyhow!("shim.init 缺失: {e}"))?
            .call1((code_dir.to_string_lossy().to_string(),))
            .map_err(|e| anyhow!("shim.init 失败（模型加载）: {e}"))?;
        tracing::info!("内嵌引擎初始化信息: {:?}", info);
        Ok(module.unbind())
    })
}

/// 以 PyConfig 隔离模式初始化解释器（标准嵌入式做法）：
/// 显式给出模块搜索路径（python310.zip / runtime / Lib / site-packages / DLLs / 代码目录），
/// 忽略 PYTHONHOME、注册表与 ._pth；UTF-8 模式开启。
/// 初始化成功后释放 GIL（等价 prepare_freethreaded_python 的行为）。
#[cfg(windows)]
unsafe fn initialize_isolated_python(runtime_dir: &Path, code_dir: &Path) -> Result<()> {
    use pyo3::ffi;

    let mut preconfig: ffi::PyPreConfig = std::mem::zeroed();
    ffi::PyPreConfig_InitIsolatedConfig(&mut preconfig);
    preconfig.utf8_mode = 1;
    if ffi::PyStatus_Exception(ffi::Py_PreInitialize(&preconfig)) != 0 {
        return Err(anyhow!("Py_PreInitialize 失败"));
    }

    let mut config: ffi::PyConfig = std::mem::zeroed();
    ffi::PyConfig_InitIsolatedConfig(&mut config);
    let search_paths = [
        runtime_dir.join("python310.zip"),
        runtime_dir.to_path_buf(),
        runtime_dir.join("Lib"),
        runtime_dir.join("Lib").join("site-packages"),
        runtime_dir.join("DLLs"),
        code_dir.to_path_buf(),
    ];
    let wide: Vec<Vec<u16>> = search_paths
        .iter()
        .map(|p| {
            p.to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect()
        })
        .collect();
    for w in &wide {
        let st = ffi::PyWideStringList_Append(&mut config.module_search_paths, w.as_ptr());
        if ffi::PyStatus_Exception(st) != 0 {
            return Err(anyhow!("PyWideStringList_Append 失败"));
        }
    }
    config.module_search_paths_set = 1;

    let status = ffi::Py_InitializeFromConfig(&config);
    ffi::PyConfig_Clear(&mut config);
    if ffi::PyStatus_Exception(status) != 0 {
        return Err(anyhow!("Py_InitializeFromConfig 失败"));
    }
    // 释放 GIL（之后统一经 with_gil 使用解释器）
    ffi::PyEval_SaveThread();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_upload_validation_rejects_unsafe_or_disguised_files() {
        assert!(validate_voice_file_name("../escape.wav").is_err());
        assert!(validate_voice_file_name("voice.exe").is_err());
        assert!(validate_voice_bytes("wav", b"not a wave file").is_err());

        let mut minimal_wav = Vec::from(&b"RIFF0000WAVE"[..]);
        minimal_wav.extend_from_slice(b"fmt ");
        assert!(validate_voice_bytes("wav", &minimal_wav).is_ok());
    }

    /// 冒烟测试：在原生宿主进程里直接走完"初始化 + 一次合成"。
    /// 运行前需把 python310.dll / python3.dll 复制到 target/release/deps。
    /// INDEXTTS_CODE_DIR / INDEXTTS_DATA_DIR 可覆盖默认路径。
    /// cargo test --release engine_embed_synth_smoke -- --ignored --nocapture
    #[test]
    #[ignore]
    fn engine_embed_synth_smoke() {
        let code_dir = std::env::var("INDEXTTS_CODE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"H:\LingChat\LingChat-rust\bin\engine"));
        let data_dir = std::env::var("INDEXTTS_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                PathBuf::from(r"H:\LingChat\LingChat-rust\bin\data\third_party\IndexTTS-AMD")
            });
        let runtime_dir = code_dir.join("runtime");
        let module = init_python(&code_dir, &runtime_dir, &data_dir, "qwen").expect("init python");
        let (reply_tx, _reply_rx) = oneshot::channel();
        let voice_path = std::fs::read_dir(data_dir.join("voices"))
            .unwrap()
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("wav"))
            .expect("voice preset");
        let job = SynthJob {
            text: "测试一下，今天也要加油喵。".to_string(),
            emotion: build_plan("qwen", "高兴", "测试一下，今天也要加油喵。"),
            voice_path,
            target: SynthTarget::Memory,
            cancelled: Arc::new(AtomicBool::new(false)),
            reply: reply_tx,
        };
        let mut cache = EmotionCache::load(data_dir.join("qwen_emo_cache.rust.test.json"));
        let SynthOutput::Bytes(bytes) = synth_blocking(&module, &job, &mut cache).expect("synth")
        else {
            panic!("unexpected file output");
        };
        assert!(bytes.len() > 1000, "wav 长度异常: {}", bytes.len());
        std::fs::write(r"H:\LingChat\synth-smoke.wav", &bytes).unwrap();
        println!("SYNTH_OK bytes={}", bytes.len());
    }
}
