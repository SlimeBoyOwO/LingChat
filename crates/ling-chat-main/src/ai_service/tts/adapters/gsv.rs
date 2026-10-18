//! GPT-SoVITS 适配器，对应 `ling_chat/core/TTS/gsv_adapter.py`。

use std::collections::HashMap;
use std::sync::{Arc, Mutex as SyncMutex};
use std::time::Instant;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{Value as JsonValue, json};
use tokio::sync::{Mutex, OnceCell};
use tokio::time::{Duration, sleep};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

/// GSV 六情绪参考语音的分类名，同时是 `gsv_emo_texts` / `gsv_emo_voice_files`
/// 的配置键。
pub const GSV_EMO_CATEGORIES: [&str; 6] = ["吃惊", "开心", "恐惧", "难过", "生气", "中立"];

/// 短句阈值：不超过该字符数（不含空白）的片段沿用上一片段的分类。
const SHORT_SENTENCE_MAX_CHARS: usize = 3;
/// 迟滞窗口：距上次合成超过该时长后不再沿用旧分类。
const SHORT_CARRY_WINDOW: Duration = Duration::from_secs(15);

/// 把情绪映射到六分类。映射已与 `utils/prompt.rs` 的提示词情绪清单对齐：
/// 慌张、担心、尴尬、紧张、高兴、自信、害怕、害羞、认真、生气、无语、
/// 厌恶、疑惑、难为情、惊讶、情动、哭泣、调皮、平静；并保留前端立绘
/// 槽位中的 兴奋/心动/无奈/正常 等变体。未识别情绪一律归入中立。
pub fn gsv_emo_category(emo: &str) -> &'static str {
    match emo {
        "惊讶" | "慌张" => "吃惊",
        // 「情动」为分类器/提示词用名，与前端立绘的「心动」同义
        "高兴" | "兴奋" | "心动" | "情动" | "调皮" | "害羞" | "自信" => "开心",
        "害怕" | "紧张" | "担心" => "恐惧",
        "哭泣" | "无奈" | "尴尬" | "无语" | "难为情" => "难过",
        "生气" | "厌恶" => "生气",
        // 正常/平静/认真/疑惑 及未识别情绪
        _ => "中立",
    }
}

/// 短句沿用上一片段的分类（迟滞），减少参考音频在短促台词间频繁跳动；
/// 距上一次真正的分类判定超过窗口则按当前情绪分类。
/// 返回 (分类, 是否沿用了上一片段的分类)——后者用于决定是否刷新迟滞锚点。
fn resolve_speak_category(
    text: &str,
    current: &str,
    last: Option<(&str, Duration)>,
) -> (String, bool) {
    if text.chars().count() > SHORT_SENTENCE_MAX_CHARS {
        return (current.to_string(), false);
    }
    match last {
        Some((cat, elapsed)) if elapsed < SHORT_CARRY_WINDOW => (cat.to_string(), true),
        _ => (current.to_string(), false),
    }
}

/// 合成文本预处理。
/// 1) 结尾不是标点时按语言显式补句末标点，避免服务端自动补句号的不确定性；
/// 2) 文本同时含中日韩字符与 ASCII 字母时返回 auto，把分词交给服务端
///    LangSegmenter，混排句不再整句按单一语言朗读。纯数字不触发（g2p 处理）。
fn preprocess_text(text: &str, text_lang: &str) -> (String, String) {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return (text.to_string(), text_lang.to_string());
    }
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str(text);
    if !ends_with_punct(trimmed) {
        let punct = match text_lang {
            "zh" | "ja" => Some('。'),
            "en" | "ko" => Some('.'),
            _ => None,
        };
        if let Some(p) = punct {
            // 标点补在尾部空白之前，不改写输入文本自身的空白
            out.insert(trimmed.len(), p);
        }
    }
    let lang = if text_lang != "auto" && contains_cjk(&out) && contains_ascii_letter(&out) {
        "auto".to_string()
    } else {
        text_lang.to_string()
    };
    (out, lang)
}

/// 结尾是否已是句末标点（含中英文常见标点与右引号）。
fn ends_with_punct(text: &str) -> bool {
    matches!(
        text.chars().last(),
        Some(
            '。' | '．'
                | '！'
                | '？'
                | '…'
                | '～'
                | '~'
                | '!'
                | '?'
                | '.'
                | '、'
                | '，'
                | ','
                | '；'
                | ';'
                | '：'
                | ':'
                | '」'
                | '』'
                | '）'
                | ')'
        )
    )
}

/// 是否含中日韩字符（含假名与谚文）。
fn contains_cjk(text: &str) -> bool {
    text.chars().any(|c| {
        matches!(
            c as u32,
            0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF
        )
    })
}

/// 是否含 ASCII 字母。
fn contains_ascii_letter(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_alphabetic())
}

/// 目标 RMS：约 -18 dBFS。按人耳感知对齐响度，比峰值对齐稳——峰值会被个别爆破音带偏。
const WAV_TARGET_RMS: f32 = 0.125;
/// 峰值上限：-1 dBFS。RMS 增益不得把样本顶到削波。
const WAV_PEAK_CEILING: f32 = 0.891;
/// 低于该 RMS 视为静音，不做增益（避免把底噪放大成噪声）。
const WAV_SILENCE_RMS: f32 = 0.004;
/// 放大上限：+6 dB。衰减不设下限，否则响度过大的片段的对齐不了。
const WAV_MAX_GAIN: f32 = 2.0;
/// 首尾静音裁剪门限：-50 dBFS。
const WAV_TRIM_THRESHOLD: f32 = 0.003;
/// 裁剪后保留的留白：句首 15 ms、句尾 60 ms，统一各句的起收节奏。
const WAV_LEAD_PAD_MS: f32 = 15.0;
const WAV_TAIL_PAD_MS: f32 = 60.0;
/// 首尾淡入淡出：5 ms，消掉裁剪点直接起振的咔哒声。
const WAV_EDGE_FADE_MS: f32 = 5.0;

/// 合成音频后处理，三件事都为了消掉逐句播放时的跳变感：
/// 1) 裁掉首尾静音再补固定留白——不同参考音频的拖尾长短不一，直接播会让句子节奏忽快忽慢；
/// 2) 按 RMS 对齐响度并限制峰值——情绪切换时不会忽大忽小，也不削波；
/// 3) 首尾 5 ms 淡入淡出——裁掉静音后直接起振会有咔哒声。
/// 只处理单声道 16-bit PCM，其他格式原样返回。
fn smooth_wav(bytes: &[u8]) -> Vec<u8> {
    let Some(wav) = parse_wav_16bit(bytes) else {
        return bytes.to_vec();
    };
    if wav.samples == 0 {
        return bytes.to_vec();
    }

    let mut samples: Vec<f32> = Vec::with_capacity(wav.samples);
    for i in 0..wav.samples {
        let offset = wav.data_start + i * 2;
        samples.push(i16::from_le_bytes([bytes[offset], bytes[offset + 1]]) as f32);
    }

    // 1) 静音裁剪：找到首个与末个有效样本，前后补固定留白
    let threshold = WAV_TRIM_THRESHOLD * i16::MAX as f32;
    let (Some(first), Some(last)) = (
        samples.iter().position(|s| s.abs() > threshold),
        samples.iter().rposition(|s| s.abs() > threshold),
    ) else {
        return bytes.to_vec(); // 整段静音，交给上层按无语音处理
    };
    let sr = wav.sample_rate as f32;
    let lead = (WAV_LEAD_PAD_MS / 1000.0 * sr) as usize;
    let tail = (WAV_TAIL_PAD_MS / 1000.0 * sr) as usize;
    let from = first.saturating_sub(lead);
    let to = (last + 1 + tail).min(samples.len());
    let mut body = samples[from..to].to_vec();

    // 2) 响度对齐：统计只取语音段，避免留白把 RMS 稀释掉
    let speech = &samples[first..=last];
    let rms = (speech.iter().map(|s| s * s).sum::<f32>() / speech.len() as f32).sqrt();
    if rms / i16::MAX as f32 >= WAV_SILENCE_RMS {
        let peak = speech.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let want = WAV_TARGET_RMS * i16::MAX as f32;
        let mut gain = (want / rms).clamp(0.1, WAV_MAX_GAIN);
        let ceiling = WAV_PEAK_CEILING * i16::MAX as f32;
        if peak * gain > ceiling {
            gain = ceiling / peak.max(1.0);
        }
        for s in body.iter_mut() {
            *s = (*s * gain).round().clamp(i16::MIN as f32, i16::MAX as f32);
        }
    }

    // 3) 首尾淡入淡出，长度不足两侧各 1 个样本时跳过
    let fade = ((WAV_EDGE_FADE_MS / 1000.0 * sr) as usize).min(body.len() / 2);
    let end = body.len().saturating_sub(1);
    for i in 0..fade {
        let k = i as f32 / fade as f32;
        body[i] *= k;
        body[end - i] *= k;
    }

    // 4) 长度变了，按单声道 16-bit PCM 重建 44 字节头，否则播放器会按旧长度读取
    let data_len = (body.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // 单声道
    out.extend_from_slice(&wav.sample_rate.to_le_bytes());
    out.extend_from_slice(&(wav.sample_rate * 2).to_le_bytes()); // 字节率
    out.extend_from_slice(&2u16.to_le_bytes()); // 块对齐
    out.extend_from_slice(&16u16.to_le_bytes()); // 位深
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in &body {
        out.extend_from_slice(&(*s as i16).to_le_bytes());
    }
    out
}

/// 单声道 16-bit PCM WAV 的解析结果。
struct WavPcm16 {
    data_start: usize,
    samples: usize,
    sample_rate: u32,
}

/// 解析单声道 16-bit PCM WAV。不支持（含多声道、浮点）或结构异常返回 None。
fn parse_wav_16bit(bytes: &[u8]) -> Option<WavPcm16> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    let mut fmt_ok = false;
    let mut sample_rate = 0u32;
    let mut data = None; // (起始偏移, 块大小)
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().ok()?) as usize;
        let body = pos + 8;
        match id {
            b"fmt " => {
                if size < 16 || body + 16 > bytes.len() {
                    return None;
                }
                // 只认单声道 PCM（格式码 1）16-bit；多声道交点样本不做后处理
                let code = u16::from_le_bytes([bytes[body], bytes[body + 1]]);
                let channels = u16::from_le_bytes([bytes[body + 2], bytes[body + 3]]);
                let bits = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]);
                fmt_ok = code == 1 && channels == 1 && bits == 16;
                sample_rate = u32::from_le_bytes(bytes[body + 4..body + 8].try_into().ok()?);
            },
            b"data" => {
                // 样本只算 data 块内的字节，块后的附加 chunk 不参与
                data = Some((body, size.min(bytes.len().saturating_sub(body))));
                break;
            },
            _ => {},
        }
        pos = body + size + (size & 1);
    }
    let (start, size) = data?;
    if !fmt_ok || sample_rate == 0 {
        return None;
    }
    Some(WavPcm16 {
        data_start: start,
        samples: size / 2,
        sample_rate,
    })
}

#[derive(Debug, Clone)]
pub struct GsvAdapter {
    api_url: String,
    ref_audio_path: String,
    prompt_text: String,
    prompt_lang: String,
    audio_format: String,
    text_lang: String,
    parallel_infer: bool,
    gpt_model_path: Option<String>,
    sovits_model_path: Option<String>,
    model_initialized: Arc<OnceCell<()>>,
    /// GPT-SoVITS/ROCm cannot safely serve several inference requests at once.
    request_lock: Arc<Mutex<()>>,
    /// 六分类参考提示（开关开启时按情绪分类实时选择）：分类 → (音频路径, 文本, 文本语言)。
    emo_prompts: HashMap<String, (String, String, String)>,
    /// 合成采样参数（角色级可配，缺省与 GPT-SoVITS 默认一致）
    temperature: f32,
    top_p: f32,
    speed_factor: f32,
    /// 上一片段实际使用的分类与时刻（短句迟滞用）。GSV 请求已串行，普通 Mutex 即可。
    last_category: Arc<SyncMutex<(Option<String>, Instant)>>,
}

impl GsvAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        api_url: String,
        ref_audio_path: String,
        prompt_text: String,
        prompt_lang: String,
        text_lang: String,
        gpt_model_path: Option<String>,
        sovits_model_path: Option<String>,
        emo_prompts: HashMap<String, (String, String, String)>,
        temperature: Option<f32>,
        top_p: Option<f32>,
        speed_factor: Option<f32>,
    ) -> Self {
        let api_url = api_url.trim_end_matches('/').to_string();
        Self {
            api_url,
            ref_audio_path,
            prompt_text,
            prompt_lang,
            audio_format: "wav".into(),
            text_lang,
            parallel_infer: true,
            gpt_model_path,
            sovits_model_path,
            model_initialized: Arc::new(OnceCell::new()),
            request_lock: Arc::new(Mutex::new(())),
            emo_prompts,
            temperature: temperature.unwrap_or(1.0).clamp(0.1, 2.0),
            top_p: top_p.unwrap_or(1.0).clamp(0.1, 1.0),
            speed_factor: speed_factor.unwrap_or(1.0).clamp(0.5, 2.0),
            last_category: Arc::new(SyncMutex::new((None, Instant::now()))),
        }
    }

    async fn ensure_model_loaded(&self) -> Result<()> {
        let (Some(gpt_model_path), Some(sovits_model_path)) =
            (&self.gpt_model_path, &self.sovits_model_path)
        else {
            return Ok(());
        };
        if gpt_model_path.trim().is_empty() || sovits_model_path.trim().is_empty() {
            return Ok(());
        }

        self.model_initialized
            .get_or_try_init(|| async {
                self.set_model(gpt_model_path, sovits_model_path).await?;
                tracing::info!(
                    gpt_model = %gpt_model_path,
                    sovits_model = %sovits_model_path,
                    "GPT-SoVITS weights loaded"
                );
                Ok::<(), anyhow::Error>(())
            })
            .await?;
        Ok(())
    }

    /// 设置 GPT + SoVITS 权重。对应 Python `set_model`。
    /// 路径按服务器视角解析（可能是远程机器），不做本地校验，加载失败由服务器返回非 200。
    pub async fn set_model(&self, gpt_model_path: &str, sovits_model_path: &str) -> Result<()> {
        let client = http_client();
        let r = client
            .get(format!("{}/set_gpt_weights", self.api_url))
            .query(&[("weights_path", gpt_model_path)])
            .send()
            .await?;
        if !r.status().is_success() {
            return Err(anyhow!("GPT 模型设置失败: HTTP {}", r.status()));
        }

        let r = client
            .get(format!("{}/set_sovits_weights", self.api_url))
            .query(&[("weights_path", sovits_model_path)])
            .send()
            .await?;
        if !r.status().is_success() {
            return Err(anyhow!("SoVITS 模型设置失败: HTTP {}", r.status()));
        }
        Ok(())
    }
}

/// 发送一次 /tts 请求；网络错误重试一次。服务端非 2xx 直接返回错误。
async fn send_tts_request(api_url: &str, body: &JsonValue) -> Result<Vec<u8>> {
    let mut retry_error = None;
    let resp = loop {
        match http_client()
            .post(format!("{api_url}/tts"))
            .json(body)
            .send()
            .await
        {
            Ok(resp) => break resp,
            Err(error) if retry_error.is_none() => {
                tracing::warn!("GPT-SoVITS request failed, retrying once: {error}");
                retry_error = Some(error);
                sleep(Duration::from_millis(500)).await;
            },
            Err(error) => {
                return Err(anyhow!(
                    "GPT-SoVITS request failed after retry: {error}; first error: {}",
                    retry_error.expect("retry error must exist")
                ));
            },
        }
    };
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("GSV 请求失败: HTTP {status}: {text}"));
    }
    Ok(resp.bytes().await?.to_vec())
}

#[async_trait]
impl TtsAdapter for GsvAdapter {
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>> {
        // VoiceMaker normally generates segments concurrently. Serialize only
        // GPT-SoVITS so Windows ROCm does not crash after the first segment.
        let _request_guard = self.request_lock.lock().await;
        self.ensure_model_loaded().await?;
        // 开关开启时按当前片段的情绪分类切换参考语音与文本；该分类在配置里
        // 没填全（不在 emo_prompts 表中）时回退默认 gsv_voice_* 配置。
        // 短句沿用上一片段的分类（迟滞），避免参考音频在短促台词间频繁跳动。
        let now = Instant::now();
        let category = {
            let mut last = self.last_category.lock().expect("last_category poisoned");
            let prev = last
                .0
                .as_ref()
                .map(|cat| (cat.as_str(), now.duration_since(last.1)));
            let (picked, carried) = resolve_speak_category(text, gsv_emo_category(emo), prev);
            // 只有真正做了新的分类判定才刷新锚点；短句沿用旧分类时保留旧时间，
            // 否则连续短句会不断续命，迟滞永不过期。
            if !carried {
                *last = (Some(picked.clone()), now);
            }
            picked
        };
        let primary = self.emo_prompts.get(&category);
        let (ref_audio_path, prompt_text, prompt_lang) = primary
            .map(|(p, t, l)| (p.as_str(), t.as_str(), l.as_str()))
            .unwrap_or((
                self.ref_audio_path.as_str(),
                self.prompt_text.as_str(),
                self.prompt_lang.as_str(),
            ));
        // 每句只用当前分类的单段参考。服务端的 aux_ref_audio_paths 是多说话人
        // 音色融合入口，会把第二段录音的参考特征一起混进音色条件；两段参考的
        // 房间声/麦克风不同时，混合结果听起来就是叠音和混响，因此不再使用。
        let (text, text_lang) = preprocess_text(text, &self.text_lang);
        let body = json!({
            "ref_audio_path": ref_audio_path,
            "prompt_text": prompt_text,
            "prompt_lang": prompt_lang,
            "text_lang": text_lang,
            "media_type": self.audio_format,
            "speed_factor": self.speed_factor,
            "text_split_method": "cut0",
            "top_k": 15,
            "top_p": self.top_p,
            "temperature": self.temperature,
            "parallel_infer": self.parallel_infer,
            "text": text,
        });
        let bytes = send_tts_request(&self.api_url, &body).await?;
        Ok(smooth_wav(&bytes))
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("api_url".into(), json!(self.api_url));
        m.insert("ref_audio_path".into(), json!(self.ref_audio_path));
        m.insert("prompt_text".into(), json!(self.prompt_text));
        m.insert("prompt_lang".into(), json!(self.prompt_lang));
        m.insert("text_lang".into(), json!(self.text_lang));
        m.insert("gpt_model_path".into(), json!(self.gpt_model_path));
        m.insert("sovits_model_path".into(), json!(self.sovits_model_path));
        m.insert("audio_format".into(), json!(self.audio_format));
        m.insert("temperature".into(), json!(self.temperature));
        m.insert("top_p".into(), json!(self.top_p));
        m.insert("speed_factor".into(), json!(self.speed_factor));
        m.insert(
            "emo_prompts".into(),
            json!(self.emo_prompts.keys().cloned().collect::<Vec<_>>()),
        );
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sampling_params_default_to_ones() {
        let adapter = GsvAdapter::new(
            "http://127.0.0.1:9880".into(),
            "ref.wav".into(),
            "你好。".into(),
            "zh".into(),
            "zh".into(),
            None,
            None,
            HashMap::new(),
            None,
            None,
            None,
        );
        let params = adapter.get_params();
        assert_eq!(params["temperature"], json!(1.0));
        assert_eq!(params["top_p"], json!(1.0));
        assert_eq!(params["speed_factor"], json!(1.0));
    }

    #[test]
    fn sampling_params_passthrough() {
        let adapter = GsvAdapter::new(
            "http://127.0.0.1:9880".into(),
            "ref.wav".into(),
            "你好。".into(),
            "zh".into(),
            "zh".into(),
            None,
            None,
            HashMap::new(),
            Some(0.8),
            Some(0.9),
            Some(1.1),
        );
        let params = adapter.get_params();
        // f32 配置值转 f64 有精度尾差，用近似比较
        let close =
            |key: &str, want: f64| (params[key].as_f64().expect("应为数字") - want).abs() < 1e-5;
        assert!(close("temperature", 0.8));
        assert!(close("top_p", 0.9));
        assert!(close("speed_factor", 1.1));
    }

    #[test]
    fn sampling_params_are_clamped() {
        // 手输越界值在后端夹取：温度 0.1–2、top_p 0.1–1、语速 0.5–2
        let adapter = GsvAdapter::new(
            "http://127.0.0.1:9880".into(),
            "ref.wav".into(),
            "你好。".into(),
            "zh".into(),
            "zh".into(),
            None,
            None,
            HashMap::new(),
            Some(0.0),
            Some(0.0),
            Some(0.0),
        );
        let params = adapter.get_params();
        let close =
            |key: &str, want: f64| (params[key].as_f64().expect("应为数字") - want).abs() < 1e-5;
        assert!(close("temperature", 0.1));
        assert!(close("top_p", 0.1));
        assert!(close("speed_factor", 0.5));

        let adapter = GsvAdapter::new(
            "http://127.0.0.1:9880".into(),
            "ref.wav".into(),
            "你好。".into(),
            "zh".into(),
            "zh".into(),
            None,
            None,
            HashMap::new(),
            Some(3.5),
            Some(3.5),
            Some(10.0),
        );
        let params = adapter.get_params();
        let close =
            |key: &str, want: f64| (params[key].as_f64().expect("应为数字") - want).abs() < 1e-5;
        assert!(close("temperature", 2.0));
        assert!(close("top_p", 1.0));
        assert!(close("speed_factor", 2.0));
    }

    #[test]
    fn short_sentence_carries_last_category() {
        // 长句按当前分类，且标记为新的分类判定
        assert_eq!(
            resolve_speak_category(
                "今天天气真不错呢",
                "开心",
                Some(("生气", Duration::from_secs(1)))
            ),
            ("开心".to_string(), false)
        );
        // 短句无历史按当前分类
        assert_eq!(
            resolve_speak_category("嗯", "开心", None),
            ("开心".to_string(), false)
        );
        // 短句沿用窗口内的历史分类
        assert_eq!(
            resolve_speak_category("嗯", "开心", Some(("生气", Duration::from_secs(2)))),
            ("生气".to_string(), true)
        );
        // 短句超出窗口后不再沿用
        assert_eq!(
            resolve_speak_category("嗯", "开心", Some(("生气", Duration::from_secs(20)))),
            ("开心".to_string(), false)
        );
    }

    #[test]
    fn emo_category_covers_all_prompt_emotions() {
        // 覆盖 utils/prompt.rs 提示词清单里的全部 19 种情绪
        let cases: &[(&str, &str)] = &[
            ("慌张", "吃惊"),
            ("惊讶", "吃惊"),
            ("担心", "恐惧"),
            ("紧张", "恐惧"),
            ("害怕", "恐惧"),
            ("高兴", "开心"),
            ("自信", "开心"),
            ("害羞", "开心"),
            ("情动", "开心"),
            ("调皮", "开心"),
            ("生气", "生气"),
            ("厌恶", "生气"),
            ("尴尬", "难过"),
            ("无语", "难过"),
            ("难为情", "难过"),
            ("哭泣", "难过"),
            ("认真", "中立"),
            ("疑惑", "中立"),
            ("平静", "中立"),
        ];
        for (emo, want) in cases {
            assert_eq!(gsv_emo_category(emo), *want, "emotion {emo}");
        }
        // 前端立绘槽位中的变体（不在提示词清单里）
        assert_eq!(gsv_emo_category("兴奋"), "开心");
        assert_eq!(gsv_emo_category("心动"), "开心");
        assert_eq!(gsv_emo_category("无奈"), "难过");
        assert_eq!(gsv_emo_category("正常"), "中立");
        // 未知情绪（分类器输出的英文等标签）回退中立
        assert_eq!(gsv_emo_category("AI思考"), "中立");
        assert_eq!(gsv_emo_category("neutral"), "中立");
    }

    #[test]
    fn preprocess_adds_ending_punctuation() {
        assert_eq!(
            preprocess_text("你好", "zh"),
            ("你好。".into(), "zh".into())
        );
        assert_eq!(
            preprocess_text("いいね", "ja"),
            ("いいね。".into(), "ja".into())
        );
        assert_eq!(
            preprocess_text("Hello", "en"),
            ("Hello.".into(), "en".into())
        );
        assert_eq!(preprocess_text("안녕", "ko"), ("안녕.".into(), "ko".into()));
        // 已有标点不重复补
        assert_eq!(
            preprocess_text("你好！", "zh"),
            ("你好！".into(), "zh".into())
        );
        assert_eq!(
            preprocess_text("Hello?", "en"),
            ("Hello?".into(), "en".into())
        );
        // 空文本与纯空白不动
        assert_eq!(preprocess_text("", "zh"), ("".into(), "zh".into()));
        assert_eq!(preprocess_text("   ", "zh"), ("   ".into(), "zh".into()));
        // 其他语言不补标点
        assert_eq!(preprocess_text("hola", "es"), ("hola".into(), "es".into()));
        // 尾部空白保留，标点补在空白之前
        assert_eq!(
            preprocess_text("你好 ", "zh"),
            ("你好。 ".into(), "zh".into())
        );
    }

    #[test]
    fn preprocess_mixed_lang_switches_to_auto() {
        // 中英混排 → auto
        assert_eq!(
            preprocess_text("这么说吧, this is fine", "zh"),
            ("这么说吧, this is fine。".into(), "auto".into())
        );
        // 纯中文不加
        assert_eq!(
            preprocess_text("今天天气不错", "zh"),
            ("今天天气不错。".into(), "zh".into())
        );
        // 纯数字不触发混排
        assert_eq!(
            preprocess_text("第3章", "zh"),
            ("第3章。".into(), "zh".into())
        );
        // 已是 auto 保持不变
        assert_eq!(
            preprocess_text("hello 你好", "auto"),
            ("hello 你好".into(), "auto".into())
        );
    }

    /// 构造 44 字节头 + 16-bit 样本的最小 WAV，供归一化测试用。
    fn make_wav(samples: &[i16]) -> Vec<u8> {
        let mut wav = vec![0u8; 44];
        wav[0..4].copy_from_slice(b"RIFF");
        wav[4..8].copy_from_slice(&((36 + samples.len() * 2) as u32).to_le_bytes());
        wav[8..12].copy_from_slice(b"WAVE");
        wav[12..16].copy_from_slice(b"fmt ");
        wav[16..20].copy_from_slice(&16u32.to_le_bytes());
        wav[20..22].copy_from_slice(&1u16.to_le_bytes()); // PCM
        wav[22..24].copy_from_slice(&1u16.to_le_bytes()); // 单声道
        wav[24..28].copy_from_slice(&44100u32.to_le_bytes());
        wav[28..32].copy_from_slice(&88200u32.to_le_bytes());
        wav[32..34].copy_from_slice(&2u16.to_le_bytes());
        wav[34..36].copy_from_slice(&16u16.to_le_bytes());
        wav[36..40].copy_from_slice(b"data");
        wav[40..44].copy_from_slice(&((samples.len() * 2) as u32).to_le_bytes());
        for s in samples {
            wav.extend_from_slice(&s.to_le_bytes());
        }
        wav
    }

    fn read_samples(wav: &[u8]) -> Vec<i16> {
        let parsed = parse_wav_16bit(wav).expect("测试 WAV 应可解析");
        (0..parsed.samples)
            .map(|i| {
                let off = parsed.data_start + i * 2;
                i16::from_le_bytes([wav[off], wav[off + 1]])
            })
            .collect()
    }

    #[test]
    fn smooth_wav_trims_edge_silence() {
        let mut samples = vec![0i16; 1000];
        samples.extend(vec![8000i16; 50]);
        samples.extend(vec![0i16; 8000]);
        let out = smooth_wav(&make_wav(&samples));
        let got = read_samples(&out);
        // 尾部大段静音被裁到固定留白，长度明显变短但语音段不丢
        assert!(got.len() < samples.len());
        assert!(got.len() > 50 + 600);
        // 淡入后首个样本为 0，不会直接起振
        assert_eq!(got[0], 0);
    }

    #[test]
    fn smooth_wav_matches_rms() {
        // 响度差 17 dB 的两句，处理后应拉到同一水平（都在 +6 dB 增益与峰值上限之内）
        let quiet = smooth_wav(&make_wav(&vec![2600i16; 4000]));
        let loud = smooth_wav(&make_wav(&vec![18000i16; 4000]));
        let rms = |wav: &[u8]| {
            let all = read_samples(wav);
            // 避开首尾淡入淡出的样本
            let mid = &all[400..all.len() - 400];
            (mid.iter().map(|v| (*v as f32).powi(2)).sum::<f32>() / mid.len() as f32).sqrt()
        };
        let (a, b) = (rms(&quiet), rms(&loud));
        assert!((a - b).abs() < 80.0, "RMS 应对齐: {a} vs {b}");
    }

    #[test]
    fn smooth_wav_limits_peak_when_rms_is_low() {
        // 个别尖峰把峰值顶高时，增益受峰值上限约束，不允许削波
        let mut samples = vec![200i16; 4000];
        samples[2000] = 32000;
        let out = smooth_wav(&make_wav(&samples));
        let peak = read_samples(&out).iter().map(|v| (*v).abs()).max().unwrap();
        assert!(peak as f32 <= WAV_PEAK_CEILING * i16::MAX as f32 + 2.0);
    }

    #[test]
    fn smooth_wav_passthrough_unsupported() {
        let junk = b"not a wav file".to_vec();
        assert_eq!(smooth_wav(&junk), junk);
        // 非 16-bit（32-bit float）原样返回
        let mut wav = make_wav(&[5000, -5000]);
        wav[34..36].copy_from_slice(&32u16.to_le_bytes());
        assert_eq!(smooth_wav(&wav), wav);
        // 多声道原样返回
        let mut stereo = make_wav(&[5000, -5000]);
        stereo[22..24].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(smooth_wav(&stereo), stereo);
        // 整段静音原样返回
        let silent = make_wav(&[0i16; 1000]);
        assert_eq!(smooth_wav(&silent), silent);
    }
}
