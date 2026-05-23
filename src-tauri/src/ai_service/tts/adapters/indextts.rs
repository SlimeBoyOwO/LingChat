//! IndexTTS2 适配器，对应 `ling_chat/core/TTS/index_adpater.py`（仅非流式）。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct IndexTtsAdapter {
    base_url: String,
    speaker_id: i32,
    audio_format: String,
    lang: String,
}

impl IndexTtsAdapter {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            speaker_id: 0,
            audio_format: "wav".into(),
            lang: "zh".into(),
        }
    }
}

impl Default for IndexTtsAdapter {
    fn default() -> Self {
        Self::new("http://127.0.0.1:23467/voice/indextts/presets".into())
    }
}

#[async_trait]
impl TtsAdapter for IndexTtsAdapter {
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>> {
        let query: Vec<(&str, String)> = vec![
            ("id", self.speaker_id.to_string()),
            ("emo_control_method", "1".into()),
            ("emo_id", emo.to_string()),
            ("vec1", "0.0".into()),
            ("vec2", "0.0".into()),
            ("vec3", "0.0".into()),
            ("vec4", "0.0".into()),
            ("vec5", "0.0".into()),
            ("vec6", "0.0".into()),
            ("vec7", "0.0".into()),
            ("vec8", "0.0".into()),
            ("emo_weight", "0.6".into()),
            ("stream", "False".into()),
            ("max_text_tokens_per_segment", "120".into()),
            ("quick_token", "0".into()),
            ("lang", self.lang.clone()),
            ("audio_format", self.audio_format.clone()),
            ("_verify", "0".into()),
            ("text", text.to_string()),
        ];
        let resp = http_client().get(&self.base_url).query(&query).send().await?;
        if !resp.status().is_success() {
            return Err(anyhow!("IndexTTS 请求失败: HTTP {}", resp.status()));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("base_url".into(), json!(self.base_url));
        m.insert("speaker_id".into(), json!(self.speaker_id));
        m.insert("audio_format".into(), json!(self.audio_format));
        m.insert("lang".into(), json!(self.lang));
        m
    }
}
