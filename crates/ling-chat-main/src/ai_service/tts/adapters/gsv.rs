//! GPT-SoVITS 适配器，对应 `ling_chat/core/TTS/gsv_adapter.py`。

use std::collections::HashMap;
use std::sync::Arc;

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

/// 把情绪映射到六分类；词典里没有的词一律归入中立。
///
/// 需要区分「词典命中的中立」和「根本不认识」时用 [`known_emo_category`]。
pub fn gsv_emo_category(emo: &str) -> &'static str {
    known_emo_category(emo).unwrap_or("中立")
}

/// 情绪词 → 六分类；**不在词典里**时返回 `None`。
///
/// 词典 = 六分类名本身（`吃惊/开心/恐惧/难过/生气/中立`，GSV 试听下拉与立绘槽位都会
/// 直接给分类名，所以这个映射对它们必须幂等）+ 情绪模型 19 类的标签名（见
/// `emotion_model_19emo/label_mapping.json`：兴奋、厌恶、哭泣、害怕、害羞、平静、
/// 心动、惊讶、慌张、担心、无奈、生气、疑惑、紧张、自信、认真、调皮、难为情、
/// 高兴）+ 提示词清单里的同义变体（慌张、担心、尴尬、紧张、高兴、自信、害怕、
/// 害羞、认真、生气、无语、厌恶、疑惑、难为情、惊讶、情动、哭泣、调皮、平静）
/// + 前端立绘槽位的变体（兴奋/心动/无奈/正常）+ 立绘别名（见
/// `docs/character-editor.md`：哭泣 ↔ 伤心、难为情 ↔ 羞耻）。
///
/// sherpa-onnx 用它判断「上游给的情绪到底认不认识」：不认识就拿正文去问
/// 情绪模型（见 `sherpa_onnx::text::resolve_emotion`）。
pub fn known_emo_category(emo: &str) -> Option<&'static str> {
    for category in GSV_EMO_CATEGORIES {
        if category == emo {
            return Some(category);
        }
    }
    Some(match emo {
        "惊讶" | "慌张" => "吃惊",
        // 「情动」为分类器/提示词用名，与前端立绘的「心动」同义
        "高兴" | "兴奋" | "心动" | "情动" | "调皮" | "害羞" | "自信" => "开心",
        "害怕" | "紧张" | "担心" => "恐惧",
        // 伤心/羞耻是立绘别名（哭泣 ↔ 伤心、难为情 ↔ 羞耻），归到同一类
        "哭泣" | "伤心" | "羞耻" | "无奈" | "尴尬" | "无语" | "难为情" => "难过",
        "生气" | "厌恶" => "生气",
        // 明确的中立词：模型标签（平静/认真/疑惑）与提示词/立绘用词（中立/正常）
        "中立" | "正常" | "平静" | "认真" | "疑惑" => "中立",
        _ => return None,
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
        let category = gsv_emo_category(emo);
        let (ref_audio_path, prompt_text, prompt_lang) = self
            .emo_prompts
            .get(category)
            .map(|(p, t, l)| (p.as_str(), t.as_str(), l.as_str()))
            .unwrap_or((
                self.ref_audio_path.as_str(),
                self.prompt_text.as_str(),
                self.prompt_lang.as_str(),
            ));
        let body = json!({
            "ref_audio_path": ref_audio_path,
            "prompt_text": prompt_text,
            "prompt_lang": prompt_lang,
            "text_lang": self.text_lang,
            "media_type": self.audio_format,
            "speed_factor": 1.0,
            "text_split_method": "cut0",
            "top_k": 15,
            "top_p": 1.0,
            "temperature": 1.0,
            "parallel_infer": self.parallel_infer,
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
        Ok(resp.bytes().await?.to_vec())
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
    fn known_emo_category_recognizes_all_model_labels() {
        // 情绪模型 19 类的标签名（label_mapping.json 的 id2label）。
        // 这 19 个词必须全部命中词典，否则 sherpa-onnx 会把模型输出当成
        // 「不认识」再拿正文去猜一遍（见 sherpa_onnx::text::resolve_emotion）。
        for label in [
            "兴奋",
            "厌恶",
            "哭泣",
            "害怕",
            "害羞",
            "平静",
            "心动",
            "惊讶",
            "慌张",
            "担心",
            "无奈",
            "生气",
            "疑惑",
            "紧张",
            "自信",
            "认真",
            "调皮",
            "难为情",
            "高兴",
        ] {
            assert!(
                known_emo_category(label).is_some(),
                "情绪模型标签 {label} 没进词典"
            );
        }
    }

    #[test]
    fn known_emo_category_recognizes_avatar_slot_emotions() {
        // 立绘差分槽位的情绪名（api/character/avatars.rs 的 EMOTIONS，去掉「头像」）
        // 与角色设置页试听下拉（VoicePreview.vue 的 emotions）里的词，都必须命中
        // 词典——否则用户明确选的情绪会被「按正文重猜」覆盖掉。
        for label in [
            "正常", "平静", "厌恶", "高兴", "担心", "生气", "紧张", "害怕", "害羞", "慌张", "认真",
            "无奈", "兴奋", "疑惑", "伤心", "心动", "调皮", "羞耻", "自信", "惊讶",
        ] {
            assert!(
                known_emo_category(label).is_some(),
                "立绘/试听里的情绪 {label} 没进词典"
            );
        }
        // 立绘别名（哭泣 ↔ 伤心、难为情 ↔ 羞耻）与本体同类
        assert_eq!(known_emo_category("伤心"), Some("难过"));
        assert_eq!(known_emo_category("羞耻"), Some("难过"));
    }

    #[test]
    fn known_emo_category_separates_neutral_from_unknown() {
        // 六分类名 → 幂等（GSV 试听下拉直接给的就是这六个词）
        for category in GSV_EMO_CATEGORIES {
            assert_eq!(known_emo_category(category), Some(category), "{category}");
            assert_eq!(gsv_emo_category(category), category, "{category}");
        }
        // 词典里的中立词 → Some("中立")；没见过的词 → None
        for neutral in ["中立", "正常", "平静", "认真", "疑惑"] {
            assert_eq!(known_emo_category(neutral), Some("中立"), "{neutral}");
        }
        for unknown in ["AI思考", "neutral", "", "快乐"] {
            assert_eq!(known_emo_category(unknown), None, "{unknown}");
        }
        // 未知词的老行为不变：分类仍是中立
        assert_eq!(gsv_emo_category("快乐"), "中立");
    }
}
