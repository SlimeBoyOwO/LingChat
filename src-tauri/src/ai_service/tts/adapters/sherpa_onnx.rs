use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};
use sherpa_onnx::{GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig, OfflineTtsVitsModelConfig};

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

        let tts = match model_type.as_str() {
            "vits" | "fastspeech2" => Self::create_vits_tts(model_dir)?,
            other => return Err(anyhow!("Sherpa-ONNX 不支持的模型类型: {other}")),
        };

        let sr = tts.sample_rate();
        tracing::info!(
            "Sherpa-ONNX 初始化完成: model_type={model_type}, language={language}, sample_rate={sr}"
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

    pub fn set_reference_audio(&mut self, samples: Vec<f32>, sample_rate: i32, text: String) {
        self.reference_audio = Some(samples);
        self.reference_sample_rate = Some(sample_rate);
        self.reference_text = Some(text);
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    fn create_vits_tts(model_dir: &Path) -> Result<OfflineTts> {
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
                ..Default::default()
            },
            max_num_sentences: 2,
            ..Default::default()
        };

        OfflineTts::create(&config)
            .ok_or_else(|| anyhow!("Sherpa-ONNX VITS 模型加载失败，请检查模型文件是否完整"))
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

    let channels = u16::from_le_bytes(data[22..24].try_into().unwrap_or([1, 0])) as u32;
    let sample_rate = i32::from_le_bytes(data[24..28].try_into().unwrap_or([0; 4]));
    let bits_per_sample = u16::from_le_bytes(data[34..36].try_into().unwrap_or([16, 0]));

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
