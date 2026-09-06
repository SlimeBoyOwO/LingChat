use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsMatchaModelConfig, OfflineTtsModelConfig, OfflineTtsVitsModelConfig,
    OfflineTtsZipvoiceModelConfig,
};

use crate::ai_service::tts::provider::TtsAdapter;
use crate::config::tts::TtsConfig;

pub struct SherpaOnnxAdapter {
    tts: Option<OfflineTts>,
    sid: i32,
    speed: f32,
    model_path: String,
    model_type: String,
    language: String,
    voice: String,
    use_gpu: bool,
    reference_audio: Option<Vec<f32>>,
    reference_sample_rate: Option<i32>,
    reference_text: Option<String>,
}

impl SherpaOnnxAdapter {
    pub fn new(
        _config: TtsConfig,
        model_path: String,
        model_type: String,
        language: String,
        voice: String,
        use_gpu: bool,
    ) -> Result<Self> {
        let model_dir = Path::new(&model_path);
        if !model_dir.exists() {
            return Err(anyhow!("Sherpa-ONNX 模型目录不存在: {model_path}"));
        }

        let provider = Self::provider_for(use_gpu);

        // 按平台选择合适的执行提供方（GPU/专用加速）；不可用或失败时回退 CPU。
        let create = |provider: Option<&str>| -> Result<OfflineTts> {
            match model_type.as_str() {
                "vits" | "fastspeech2" => Self::create_vits_tts(model_dir, provider),
                "matcha" => Self::create_matcha_tts(model_dir, provider),
                "kokoro" => Self::create_kokoro_tts(model_dir, &language, provider),
                "zipvoice" => Self::create_zipvoice_tts(model_dir, provider),
                other => return Err(anyhow!("Sherpa-ONNX 不支持的模型类型: {other}")),
            }
        };

        let tts = match create(provider) {
            Ok(tts) => {
                if let Some(p) = provider {
                    tracing::info!("Sherpa-ONNX 使用推理后端: {p}");
                }
                tts
            }
            Err(e) => match provider {
                // GPU/专用后端初始化失败时静默回退到 CPU，保证可用性。
                Some(_) => {
                    tracing::warn!(
                        "Sherpa-ONNX 后端初始化失败（{e}），回退到 CPU 推理"
                    );
                    create(None)?
                }
                None => return Err(e),
            },
        };

        let sr = tts.sample_rate();
        tracing::info!(
            "Sherpa-ONNX 初始化完成: model_type={model_type}, language={language}, provider={:?}, sample_rate={sr}",
            provider.unwrap_or("cpu")
        );

        Ok(Self {
            tts: Some(tts),
            sid: 0,
            speed: 1.0,
            model_path,
            model_type,
            language,
            voice,
            use_gpu,
            reference_audio: None,
            reference_sample_rate: None,
            reference_text: None,
        })
    }

    /// 根据是否开启 GPU 加速与目标平台，选择 ONNX Runtime 执行提供方。
    ///
    /// 返回 `None` 表示使用 CPU。各平台：
    /// - Android: `nnapi`（走设备 GPU/NPU）
    /// - Windows: `dml`（DirectML，无需单独装 CUDA 工具链）
    /// - macOS: `coreml`
    /// - Linux x86_64: `cuda`（需 NVIDIA 驱动/运行时，不可用会自动回退 CPU）
    /// - 其它: CPU
    fn provider_for(use_gpu: bool) -> Option<&'static str> {
        if !use_gpu {
            return None;
        }
        #[cfg(target_os = "android")]
        {
            return Some("nnapi");
        }
        #[cfg(target_os = "windows")]
        {
            return Some("dml");
        }
        #[cfg(target_os = "macos")]
        {
            return Some("coreml");
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            return Some("cuda");
        }
        #[allow(unreachable_code)]
        None
    }

    pub fn set_reference_audio(&mut self, samples: Vec<f32>, sample_rate: i32, text: String) {
        self.reference_audio = Some(samples);
        self.reference_sample_rate = Some(sample_rate);
        self.reference_text = Some(text);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    fn create_vits_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let model_file = find_file(model_dir, &["model.onnx", "tts-model.onnx", "sherpa-onnx-tts.onnx"])?;
        let tokens_file = find_file(model_dir, &["tokens.txt"])?;

        let vits_config = OfflineTtsVitsModelConfig {
            model: Some(model_file.to_string_lossy().to_string()),
            lexicon: find_file_optional(model_dir, &["lexicon.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            tokens: Some(tokens_file.to_string_lossy().to_string()),
            data_dir: find_dir_optional(model_dir, &["data-dir", "data"])
                .map(|p| p.to_string_lossy().to_string()),
            noise_scale: 0.667,
            noise_scale_w: 0.8,
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict-dir", "dict"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                vits: vits_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("Sherpa-ONNX VITS 模型加载失败，请检查模型文件是否完整"))
    }

    /// 返回包含 `espeak-ng-data` 子目录的父目录（即 data_dir 的取值）。
    fn espeak_data_dir(model_dir: &Path) -> Option<PathBuf> {
        find_dir_optional(model_dir, &["espeak-ng-data"])
            .map(|p| p.parent().unwrap_or(model_dir).to_path_buf())
    }

    fn create_matcha_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let acoustic_model =
            find_file(model_dir, &["model.onnx", "model-steps-3.onnx", "model-steps-6.onnx"])?;
        let tokens_file = find_file(model_dir, &["tokens.txt"])?;
        let vocoder = find_file(
            model_dir,
            &[
                "vocos-22khz-univ.onnx",
                "vocos-16khz-univ.onnx",
                "hifigan_v3.onnx",
                "mb_melgan.onnx",
                "vocoder.onnx",
            ],
        )?;

        let matcha_config = OfflineTtsMatchaModelConfig {
            acoustic_model: Some(acoustic_model.to_string_lossy().to_string()),
            vocoder: Some(vocoder.to_string_lossy().to_string()),
            lexicon: find_file_optional(model_dir, &["lexicon.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            tokens: Some(tokens_file.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir)
                .map(|p| p.to_string_lossy().to_string()),
            noise_scale: 0.667,
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict"])
                .map(|p| p.to_string_lossy().to_string()),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                matcha: matcha_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("Sherpa-ONNX Matcha 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_kokoro_tts(
        model_dir: &Path,
        language: &str,
        provider: Option<&str>,
    ) -> Result<OfflineTts> {
        let model = find_file(model_dir, &["model.onnx", "model.int8.onnx"])?;
        let voices = find_file(model_dir, &["voices.bin"])?;
        // kokoro-int8-multi-lang-v1_1 支持 auto/zh/en 等 lang；默认交给模型自动判断。
        let lang = match language {
            "zh" | "en" | "ja" => language.to_string(),
            _ => "auto".to_string(),
        };

        let kokoro_config = OfflineTtsKokoroModelConfig {
            model: Some(model.to_string_lossy().to_string()),
            voices: Some(voices.to_string_lossy().to_string()),
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            data_dir: Self::espeak_data_dir(model_dir)
                .map(|p| p.to_string_lossy().to_string()),
            length_scale: 1.0,
            dict_dir: find_dir_optional(model_dir, &["dict"])
                .map(|p| p.to_string_lossy().to_string()),
            lexicon: None,
            lang: Some(lang),
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                kokoro: kokoro_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("Sherpa-ONNX Kokoro 模型加载失败，请检查模型文件是否完整"))
    }

    fn create_zipvoice_tts(model_dir: &Path, provider: Option<&str>) -> Result<OfflineTts> {
        let encoder = find_file(model_dir, &["text_encoder.onnx"])?;
        let decoder = find_file(model_dir, &["fm_decoder.onnx"])?;
        let vocoder = find_file(model_dir, &["vocos_24khz.onnx", "vocoder.onnx"])?;

        let zipvoice_config = OfflineTtsZipvoiceModelConfig {
            tokens: find_file_optional(model_dir, &["tokens.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            encoder: Some(encoder.to_string_lossy().to_string()),
            decoder: Some(decoder.to_string_lossy().to_string()),
            vocoder: Some(vocoder.to_string_lossy().to_string()),
            // ZipVoice 的 data-dir 必须是直接包含 phontab/phondata/phonindex 的目录
            //（与 Matcha/espeak 的 espeak-ng-data 父目录层级不同）。部分模型把
            // 这些语音库文件放到 espeak-ng-data/ 下，因此需按 phontab 实际所在目录解析。
            data_dir: zipvoice_data_dir(model_dir)
                .map(|p| p.to_string_lossy().to_string()),
            lexicon: find_file_optional(model_dir, &["lexicon.txt"])
                .map(|p| p.to_string_lossy().to_string()),
            feat_scale: 10.0,
            t_shift: 0.005,
            target_rms: 0.25,
            guidance_scale: 1.0,
        };

        let config = OfflineTtsConfig {
            model: OfflineTtsModelConfig {
                zipvoice: zipvoice_config,
                provider: provider.map(|s| s.to_string()),
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("Sherpa-ONNX ZipVoice 模型加载失败，请检查模型文件是否完整"))
    }
}

#[async_trait]
impl TtsAdapter for SherpaOnnxAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let tts = self
            .tts
            .as_ref()
            .ok_or_else(|| anyhow!("Sherpa-ONNX 引擎未初始化"))?;

        let gen_config = GenerationConfig {
            sid: self.sid,
            speed: self.speed,
            reference_audio: self.reference_audio.clone(),
            reference_sample_rate: self.reference_sample_rate.unwrap_or(0),
            reference_text: self.reference_text.clone(),
            ..Default::default()
        };

        let audio = tts
            .generate_with_config(text, &gen_config, None::<fn(&[f32], f32) -> bool>)
            .ok_or_else(|| anyhow!("Sherpa-ONNX 生成失败"))?;

        let samples = audio.samples().to_vec();
        let sample_rate = audio.sample_rate();

        if samples.is_empty() {
            return Err(anyhow!("Sherpa-ONNX 生成的音频为空"));
        }

        Ok(f32_to_wav(&samples, sample_rate))
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut params = HashMap::new();
        params.insert("model_path".into(), json!(self.model_path));
        params.insert("model_type".into(), json!(self.model_type));
        params.insert("language".into(), json!(self.language));
        params.insert("voice".into(), json!(self.voice));
        params.insert("use_gpu".into(), json!(self.use_gpu));
        params.insert("speed".into(), json!(self.speed));
        params.insert("sid".into(), json!(self.sid));
        params.insert("has_reference_audio".into(), json!(self.reference_audio.is_some()));
        params
    }
}

impl std::fmt::Debug for SherpaOnnxAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SherpaOnnxAdapter")
            .field("model_path", &self.model_path)
            .field("model_type", &self.model_type)
            .field("language", &self.language)
            .field("voice", &self.voice)
            .field("use_gpu", &self.use_gpu)
            .field("sid", &self.sid)
            .field("speed", &self.speed)
            .field("has_reference_audio", &self.reference_audio.is_some())
            .finish()
    }
}

fn find_file(dir: &Path, candidates: &[&str]) -> Result<PathBuf> {
    for name in candidates {
        let path = dir.join(name);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "在 {} 中未找到模型文件，候选: {:?}",
        dir.display(),
        candidates
    ))
}

fn find_file_optional(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.exists())
}

fn find_dir_optional(dir: &Path, candidates: &[&str]) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|name| dir.join(name))
        .find(|p| p.is_dir())
}

/// 定位 ZipVoice 数据目录：该目录需直接包含 phontab/phondata/phonindex。
/// 优先匹配目录本身或其 `espeak-ng-data` 子目录，其次匹配模型目录。
fn zipvoice_data_dir(model_dir: &Path) -> Option<PathBuf> {
    if model_dir.join("phontab").exists() {
        return Some(model_dir.to_path_buf());
    }
    if find_dir_optional(model_dir, &["espeak-ng-data"])
        .is_some_and(|p| p.join("phontab").exists())
    {
        return find_dir_optional(model_dir, &["espeak-ng-data"]);
    }
    None
}

fn f32_to_wav(samples: &[f32], sample_rate: i32) -> Vec<u8> {
    let num_samples = samples.len();
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;
    let sr = sample_rate as u32;
    let byte_rate = sr * 2;

    let mut wav = Vec::with_capacity(44 + data_size);

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(file_size as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&sr.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());

    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(data_size as u32).to_le_bytes());

    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let i16_sample = (clamped * 32767.0) as i16;
        wav.extend_from_slice(&i16_sample.to_le_bytes());
    }

    wav
}

pub fn load_reference_audio(path: &Path) -> Result<(Vec<f32>, i32)> {
    let data = std::fs::read(path)
        .map_err(|e| anyhow!("无法读取参考音频文件 {}: {}", path.display(), e))?;

    // 最小 WAV 头：44 字节（RIFF/WAVE/fmt/data 头）。低于此长度的输入直接报错，
    // 避免下方切片越界 panic。
    if data.len() < 44 {
        return Err(anyhow!(
            "WAV 文件过短（{} 字节，至少需要 44 字节）: {}",
            data.len(),
            path.display()
        ));
    }

    let channels = u16::from_le_bytes(data[22..24].try_into().unwrap()) as u32;
    let sample_rate = i32::from_le_bytes(data[24..28].try_into().unwrap());
    let bits_per_sample = u16::from_le_bytes(data[34..36].try_into().unwrap());

    let data_offset = find_wav_data_chunk(&data).ok_or_else(|| anyhow!("WAV 文件中未找到 data 块: {}", path.display()))?;

    let samples = match bits_per_sample {
        16 => {
            let raw = &data[data_offset..];
            raw.chunks_exact(2)
                .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
                .collect::<Vec<_>>()
        }
        32 => {
            let raw = &data[data_offset..];
            raw.chunks_exact(4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2147483648.0)
                .collect::<Vec<_>>()
        }
        _ => return Err(anyhow!("不支持的 WAV 位深度: {} (仅支持 16/32)", bits_per_sample)),
    };

    let mono_samples = if channels > 1 {
        samples.chunks(channels as usize)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    } else {
        samples
    };

    if mono_samples.is_empty() {
        return Err(anyhow!("参考音频为空"));
    }

    Ok((mono_samples, sample_rate))
}

fn find_wav_data_chunk(data: &[u8]) -> Option<usize> {
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().ok()?) as usize;
        if chunk_id == b"data" {
            return Some(pos + 8);
        }
        pos += 8 + chunk_size;
        if pos % 2 != 0 {
            pos += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_model_dir() -> String {
        std::env::var("SHERPA_TEST_MODEL_DIR").unwrap_or_default()
    }

    /// 解析 WAV 字节并返回 (采样率, 单声道样本数)。
    fn parse_wav(wav: &[u8]) -> (u32, usize) {
        assert!(wav.len() >= 44, "WAV 太短");
        assert_eq!(&wav[0..4], b"RIFF", "缺 RIFF 头");
        assert_eq!(&wav[8..12], b"WAVE", "缺 WAVE");
        let sample_rate = u32::from_le_bytes(wav[24..28].try_into().unwrap());
        let bits = u16::from_le_bytes(wav[34..36].try_into().unwrap());
        let data_start = find_wav_data_chunk(wav).expect("缺 data chunk");
        let data_bytes = wav.len() - data_start;
        (sample_rate, data_bytes / (bits as usize / 8))
    }

    /// ZipVoice 零样本语音克隆：加载参考音频 → 设置 → 合成 → 验证非静音。
    ///
    /// 依赖真实模型和参考音频，默认忽略；通过设置 `SHERPA_TEST_MODEL_DIR`（模型目录）
    /// 并用 `-- --ignored` 显式运行。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "需要 zipvoice 模型目录（SHERPA_TEST_MODEL_DIR）"]
    async fn zipvoice_clone_with_reference() {
        let model_dir = test_model_dir();
        assert!(
            !model_dir.is_empty() && Path::new(&model_dir).exists(),
            "模型目录不存在，请设置 SHERPA_TEST_MODEL_DIR: {model_dir}"
        );

        let ref_path = std::env::var("SHERPA_TEST_REF_AUDIO").unwrap_or_else(|_| {
            "/tmp/baizi_ref.wav".to_string()
        });
        assert!(
            Path::new(&ref_path).exists(),
            "参考音频不存在: {ref_path}"
        );

        // 加载参考音频
        let (samples, sample_rate) = load_reference_audio(Path::new(&ref_path))
            .expect("加载参考音频失败");
        eprintln!(
            "参考音频: sr={}, samples={} ({:.2}s)",
            sample_rate,
            samples.len(),
            samples.len() as f64 / sample_rate as f64
        );

        // 初始化 adapter
        let adapter = SherpaOnnxAdapter::new(
            TtsConfig::default(),
            model_dir,
            "zipvoice".to_string(),
            "zh".to_string(),
            "female".to_string(),
            false,
        )
        .expect("初始化 Sherpa-ONNX 失败");

        // 设置参考音频
        let mut adapter = adapter;
        adapter.set_reference_audio(samples, sample_rate, "你好，我是白子。".to_string());
        adapter.set_speed(1.0);

        // 合成
        let text = "老师好，今天也请多指教了。";
        eprintln!("合成文本: {text}");
        let wav = adapter
            .generate_voice(text, "")
            .await
            .expect("合成失败");

        let (sr, num_samples) = parse_wav(&wav);
        eprintln!("输出: sr={sr}, 样本数={num_samples}, WAV-Ok");

        // 验证音频非静音
        let data_start = find_wav_data_chunk(&wav).unwrap();
        let pcm = &wav[data_start..];
        let has_signal = pcm
            .chunks_exact(2)
            .any(|c| i16::from_le_bytes([c[0], c[1]]).unsigned_abs() > 200);
        assert!(has_signal, "生成音频疑似全静音");

        // 保存输出
        let out = std::env::temp_dir().join("sherpa_clone_output.wav");
        std::fs::write(&out, &wav).expect("写入输出失败");
        eprintln!("输出文件: {}", out.display());
    }

    /// 完整跑一遍 Matcha 语音合成（覆盖初始化 + generate_voice + WAV 编码），
    /// 分别验证 CPU 与 GPU(此机无 NVIDIA -> CUDA 失败回退 CPU) 两条路径。
    ///
    /// 依赖真实模型，默认忽略；通过设置 `SHERPA_TEST_MODEL_DIR`（模型目录）
    /// 并用 `-- --ignored` 显式运行。
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "需要真实 sherpa 模型目录（SHERPA_TEST_MODEL_DIR）"]
    async fn full_matcha_synthesis() {
        let model_dir = test_model_dir();
        assert!(
            !model_dir.is_empty() && Path::new(&model_dir).exists(),
            "模型目录不存在，请设置 SHERPA_TEST_MODEL_DIR: {model_dir}"
        );

        for use_gpu in [false, true] {
            let adapter = SherpaOnnxAdapter::new(
                TtsConfig::default(),
                model_dir.clone(),
                "matcha".to_string(),
                "zh".to_string(),
                "female".to_string(),
                use_gpu,
            )
            .unwrap_or_else(|e| panic!("use_gpu={use_gpu} 初始化失败: {e}"));

            let wav = adapter
                .generate_voice("你好，世界。这是一段语音合成测试。", "")
                .await
                .expect("生成失败");

            let (sr, samples) = parse_wav(&wav);
            assert_eq!(sr, 22050, "Matcha-zh 采样率应为 22050");
            assert!(samples >= 5, "生成音频过短");

            let data_start = find_wav_data_chunk(&wav).unwrap();
            let pcm = &wav[data_start..];
            let has_signal = pcm.chunks_exact(2).any(|c| {
                let v = i16::from_le_bytes([c[0], c[1]]);
                v.unsigned_abs() > 200
            });
            assert!(has_signal, "use_gpu={use_gpu} 生成音频疑似全静音");
            eprintln!("use_gpu={use_gpu}: sr={sr}, 样本数={samples}, WAV-Ok");
        }
    }
}
