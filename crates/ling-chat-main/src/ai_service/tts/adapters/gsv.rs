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
/// 距上次合成超过窗口则按当前情绪分类。
fn resolve_speak_category(text: &str, current: &str, last: Option<(&str, Duration)>) -> String {
    if text.chars().count() > SHORT_SENTENCE_MAX_CHARS {
        return current.to_string();
    }
    match last {
        Some((cat, elapsed)) if elapsed < SHORT_CARRY_WINDOW => cat.to_string(),
        _ => current.to_string(),
    }
}

/// 主参考非中立且配置了中立分类时，把中立音频作为辅助参考（inp_refs）
/// 一起交给服务端做音色融合，让相邻句共享音色底子、降低情绪切换的突变感。
fn resolve_aux_refs<'a>(
    category: &str,
    emo_prompts: &'a HashMap<String, (String, String, String)>,
) -> Vec<&'a str> {
    if category == "中立" {
        return Vec::new();
    }
    emo_prompts
        .get("中立")
        .map(|(path, _, _)| vec![path.as_str()])
        .unwrap_or_default()
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

/// 目标峰值：-3 dBFS，乘性对齐后段落间响度一致。
const WAV_TARGET_PEAK: f32 = 23173.0 / 32767.0;
/// 低于该峰值视为静音，原样返回（避免把底噪放大成噪声）。
const WAV_SILENCE_PEAK: f32 = 0.004;
/// 放大上限：+6 dB。
const WAV_MAX_GAIN: f32 = 2.0;

/// WAV 峰值对齐：不同参考音频带出的响度差会让连续播放忽大忽小，
/// 这里把每段合成音频统一拉到同一峰值。只做乘性增益，不压缩段内动态；
/// 仅处理 16-bit PCM，其他格式原样返回。
fn normalize_wav_peak(bytes: &[u8]) -> Vec<u8> {
    let Some((data_start, sample_count)) = parse_wav_16bit(bytes) else {
        return bytes.to_vec();
    };
    if sample_count == 0 {
        return bytes.to_vec();
    }
    let mut peak = 0i16;
    for i in 0..sample_count {
        let offset = data_start + i * 2;
        let s = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let abs = s.saturating_abs();
        if abs > peak {
            peak = abs;
        }
    }
    let peak_ratio = peak as f32 / i16::MAX as f32;
    if peak_ratio < WAV_SILENCE_PEAK {
        return bytes.to_vec();
    }
    let gain = (WAV_TARGET_PEAK / peak_ratio).min(WAV_MAX_GAIN);
    if (gain - 1.0).abs() < 0.02 {
        return bytes.to_vec();
    }
    let mut out = bytes.to_vec();
    for i in 0..sample_count {
        let offset = data_start + i * 2;
        let s = i16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
        let scaled = (s as f32 * gain)
            .round()
            .clamp(i16::MIN as f32, i16::MAX as f32) as i16;
        out[offset..offset + 2].copy_from_slice(&scaled.to_le_bytes());
    }
    out
}

/// 解析 16-bit PCM WAV，返回 (data 起始偏移, 样本数)。不支持或结构异常返回 None。
fn parse_wav_16bit(bytes: &[u8]) -> Option<(usize, usize)> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut pos = 12usize;
    let mut is_16bit = false;
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
                is_16bit = u16::from_le_bytes([bytes[body + 14], bytes[body + 15]]) == 16;
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
    if !is_16bit {
        return None;
    }
    Some((start, size / 2))
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
            temperature: temperature.unwrap_or(1.0),
            top_p: top_p.unwrap_or(1.0),
            speed_factor: speed_factor.unwrap_or(1.0),
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
            let picked = resolve_speak_category(text, gsv_emo_category(emo), prev);
            *last = (Some(picked.clone()), now);
            picked
        };
        let (ref_audio_path, prompt_text, prompt_lang) = self
            .emo_prompts
            .get(&category)
            .map(|(p, t, l)| (p.as_str(), t.as_str(), l.as_str()))
            .unwrap_or((
                self.ref_audio_path.as_str(),
                self.prompt_text.as_str(),
                self.prompt_lang.as_str(),
            ));
        let inp_refs = resolve_aux_refs(&category, &self.emo_prompts);
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
            "inp_refs": inp_refs,
            "text": text,
        });
        let mut retry_error = None;
        let resp = loop {
            match http_client()
                .post(format!("{}/tts", self.api_url))
                .json(&body)
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
        let bytes = resp.bytes().await?.to_vec();
        Ok(normalize_wav_peak(&bytes))
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
    fn short_sentence_carries_last_category() {
        // 长句按当前分类
        assert_eq!(
            resolve_speak_category(
                "今天天气真不错呢",
                "开心",
                Some(("生气", Duration::from_secs(1)))
            ),
            "开心"
        );
        // 短句无历史按当前分类
        assert_eq!(resolve_speak_category("嗯", "开心", None), "开心");
        // 短句沿用窗口内的历史分类
        assert_eq!(
            resolve_speak_category("嗯", "开心", Some(("生气", Duration::from_secs(2)))),
            "生气"
        );
        // 短句超出窗口后不再沿用
        assert_eq!(
            resolve_speak_category("嗯", "开心", Some(("生气", Duration::from_secs(20)))),
            "开心"
        );
    }

    #[test]
    fn aux_refs_uses_neutral_only() {
        let mut map = HashMap::new();
        map.insert(
            "中立".into(),
            ("neutral.wav".into(), "平静。".into(), "zh".into()),
        );
        map.insert(
            "开心".into(),
            ("happy.wav".into(), "好耶。".into(), "zh".into()),
        );
        assert_eq!(resolve_aux_refs("开心", &map), vec!["neutral.wav"]);
        assert!(resolve_aux_refs("中立", &map).is_empty());
        assert!(resolve_aux_refs("开心", &HashMap::new()).is_empty());
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
        let (start, count) = parse_wav_16bit(wav).expect("测试 WAV 应可解析");
        (0..count)
            .map(|i| i16::from_le_bytes([wav[start + i * 2], wav[start + i * 2 + 1]]))
            .collect()
    }

    #[test]
    fn wav_peak_normalize_pulls_down() {
        let wav = make_wav(&[20000, -20000, 30000, -30000]);
        let out = normalize_wav_peak(&wav);
        let samples = read_samples(&out);
        let peak = samples.iter().map(|v| v.abs()).max().unwrap();
        // 峰值对齐到 23173（-3 dBFS）附近
        assert!((peak as i32 - 23173).abs() <= 2);
        // 相对比例保持
        let want = (20000f32 / 30000f32 * peak as f32).round() as i16;
        assert!((samples[0] - want).abs() <= 1);
    }

    #[test]
    fn wav_peak_normalize_caps_gain_and_skips_silence() {
        // 放大有上限：3000 峰值只放大到 6000（+6 dB）
        let small = make_wav(&[3000, -3000]);
        assert_eq!(read_samples(&normalize_wav_peak(&small)), vec![6000, -6000]);
        // 极小声视为静音，原样返回
        let quiet = make_wav(&[50, -50]);
        assert_eq!(normalize_wav_peak(&quiet), quiet);
    }

    #[test]
    fn wav_peak_normalize_passthrough_non_wav() {
        let junk = b"not a wav file".to_vec();
        assert_eq!(normalize_wav_peak(&junk), junk);
        // 非 16-bit 的 WAV（32-bit float）原样返回
        let mut wav = make_wav(&[100, -100]);
        wav[34..36].copy_from_slice(&32u16.to_le_bytes());
        assert_eq!(normalize_wav_peak(&wav), wav);
    }
}
