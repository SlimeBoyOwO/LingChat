//! 本地离线 ASR provider：sherpa-onnx OfflineRecognizer（SenseVoice）。
//!
//! 与云端 provider 不同，这个 provider **不需要任何凭据或配置字段**——模型从
//! 与 TTS 共享的 `sherpa_onnx_models` 目录就地加载（目录里的 SenseVoice 模型
//! 也是从 SettingsSherpa 下载页装下来的），首次识别时懒加载并缓存引擎。
//!
//! 用途：微调数据集自动标注。数据是预录固定音频，用一次性离线识别最合适，
//! 不涉及流式与端点检测，所以只实现 [`AsrProvider::recognize`]，不走云端会话
//! 链路。识别输入约定与其它 provider 一致：16kHz mono PCM16 WAV 字节。

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineSenseVoiceModelConfig,
};

use crate::ai_service::asr::provider_meta::AsrConfigField;
use crate::ai_service::asr::error::AsrError;
use crate::ai_service::asr::provider::AsrProvider;
use crate::ai_service::asr::provider_types::{AsrOptions, AsrResult};

/// 目录名里含该关键词即识别为 SenseVoice（目录含版本号等后缀变体时不至于找不到）。
const SENSE_VOICE_DIR_KEY: &str = "sherpa-onnx-sense-voice";

/// 加载失败后重建引擎的冷却周期：失败后至少隔这么多「次识别尝试」才重试，
/// 避免每次都反复尝试创建引擎（模型没装的时候白白空转）。
const REBUILD_COOLDOWN: usize = 32;

/// 本地 sherpa-onnx 离线识别 provider。
pub struct SherpaAsrProvider {
    /// 缓存的识别引擎。`None` 表示尚未加载或加载失败。
    engine: Mutex<Option<OfflineRecognizer>>,
    /// 自上次加载尝试以来的识别次数，用于节流失败后的重建尝试。
    attempts_since_load_attempt: AtomicUsize,
}

impl Default for SherpaAsrProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SherpaAsrProvider {
    pub const ID: &'static str = "sherpa-asr";
    pub const DISPLAY: &'static str = "本地 sherpa-onnx（SenseVoice）";

    pub fn new() -> Self {
        Self {
            engine: Mutex::new(None),
            attempts_since_load_attempt: AtomicUsize::new(0),
        }
    }

    /// 是否已下载 SenseVoice 模型（用于自动标注时「本地优先、云端回退」判断）。
    pub fn model_installed() -> bool {
        Self::resolve_model_paths().is_some()
    }

    /// 检查共享模型目录里是否已下载 SenseVoice 模型，合并出识别所需路径。
    fn resolve_model_paths() -> Option<(PathBuf, PathBuf)> {
        let root = crate::ai_service::tts::local::sherpa_onnx_manager::sherpa_onnx_models_root();
        for entry in std::fs::read_dir(&root).ok()? {
            let entry = entry.ok()?;
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.contains(SENSE_VOICE_DIR_KEY) {
                continue;
            }
            // SenseVoice 目录结构：model.int8.onnx（或 model.onnx）+ tokens.txt。
            let model = ["model.int8.onnx", "model.onnx"]
                .into_iter()
                .map(|f| entry.path().join(f))
                .find(|p| p.exists())?;
            let tokens = entry.path().join("tokens.txt");
            if tokens.exists() {
                return Some((model, tokens));
            }
        }
        None
    }

    /// 尝试构建引擎；仅在模型目录存在且文件齐全时成功。
    fn build_engine() -> Option<OfflineRecognizer> {
        let (model, tokens) = Self::resolve_model_paths()?;
        let mut config = OfflineRecognizerConfig::default();
        config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
            model: Some(model.to_string_lossy().into_owned()),
            language: Some("auto".into()),
            use_itn: true,
        };
        config.model_config.tokens = Some(tokens.to_string_lossy().into_owned());
        config.model_config.model_type = Some("sense_voice".into());
        config.model_config.num_threads = 4;
        OfflineRecognizer::create(&config)
    }
}

/// 把 16kHz mono PCM16 WAV 字节解码成 f32 样本（范围 -1..1）与采样率。
fn decode_wav(wav_bytes: &[u8]) -> Result<(Vec<f32>, u32), AsrError> {
    let mut reader = hound::WavReader::new(Cursor::new(wav_bytes))
        .map_err(|e| AsrError::InvalidAudioFormat(format!("解析 WAV 失败: {e}")))?;
    let spec = reader.spec();
    let interleaved: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| AsrError::InvalidAudioFormat(format!("读取 WAV 失败: {e}")))?,
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample.saturating_sub(1))) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|e| AsrError::InvalidAudioFormat(format!("读取 WAV 失败: {e}")))?
        },
    };
    let channels = spec.channels.max(1) as usize;
    if channels == 1 {
        Ok((interleaved, spec.sample_rate))
    } else {
        Ok((
            interleaved
                .chunks(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                .collect(),
            spec.sample_rate,
        ))
    }
}

#[async_trait]
impl AsrProvider for SherpaAsrProvider {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn display_name(&self) -> &'static str {
        Self::DISPLAY
    }

    fn config_fields(&self) -> Vec<AsrConfigField> {
        // 本地 provider 零配置：模型从共享下载页装好后即用。返回空字段，
        // 前端据此不渲染任何配置输入框。
        Vec::new()
    }

    fn supports_streaming(&self) -> bool {
        false
    }

    async fn recognize(
        &self,
        wav_bytes: Vec<u8>,
        _opts: &AsrOptions,
    ) -> Result<AsrResult, AsrError> {
        // 解码（heavy，拉到阻塞线程）。
        let (samples, sample_rate) = tauri::async_runtime::spawn_blocking({
            let wav_bytes = wav_bytes.clone();
            move || decode_wav(&wav_bytes)
        })
        .await
        .map_err(|e| AsrError::EngineLoadFailed(format!("解码任务失败: {e}")))??;

        // 取引擎（take-out）：有缓存直接拿；没有则按冷却节流尝试创建。
        let engine = {
            let mut guard = self
                .engine
                .lock()
                .map_err(|_| AsrError::EngineLoadFailed("引擎锁中毒".into()))?;
            let cached = guard.take();
            if cached.is_some() {
                cached
            } else {
                let attempts = self.attempts_since_load_attempt.fetch_add(1, Ordering::Relaxed);
                if attempts % REBUILD_COOLDOWN == 0 {
                    let built = Self::build_engine();
                    if built.is_some() {
                        self.attempts_since_load_attempt.store(0, Ordering::Relaxed);
                    }
                    built
                } else {
                    None
                }
            }
        };
        let Some(engine) = engine else {
            return Err(AsrError::EngineLoadFailed(
                "未找到本地 SenseVoice 模型，请先在模型下载页安装".into(),
            ));
        };

        // 推理（heavy）：把引擎搬进阻塞线程，跑完连同结果一起带回来再放回缓存。
        let run = tauri::async_runtime::spawn_blocking(move || {
            let stream = engine.create_stream();
            stream.accept_waveform(sample_rate as i32, &samples);
            engine.decode(&stream);
            let text = stream.get_result().map(|r| r.text).unwrap_or_default();
            (engine, text)
        })
        .await
        .map_err(|e| AsrError::EngineLoadFailed(format!("识别任务失败: {e}")))?;
        let (engine, text) = run;

        if let Ok(mut guard) = self.engine.lock() {
            *guard = Some(engine);
        }
        if text.trim().is_empty() {
            return Err(AsrError::ProviderApiError {
                provider: Self::ID.into(),
                message: "识别结果为空".into(),
            });
        }

        Ok(AsrResult {
            text,
            confidence: None,
            provider_id: Self::ID.into(),
        })
    }
}