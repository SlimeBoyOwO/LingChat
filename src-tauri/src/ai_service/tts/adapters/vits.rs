//! Simple-Vits-API VITS 适配器，对应 `ling_chat/core/TTS/vits_adapter.py`。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct VitsAdapter {
    api_url: String,
    speaker_id: i32,
    audio_format: String,
    lang: String,
}

impl VitsAdapter {
    pub fn new(api_url: String, speaker_id: i32, audio_format: String, lang: String) -> Self {
        let api_url = api_url.trim_end_matches('/').to_string();
        Self {
            api_url,
            speaker_id,
            audio_format,
            lang,
        }
    }
}

#[async_trait]
impl TtsAdapter for VitsAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let query: Vec<(&str, String)> = vec![
            ("id", self.speaker_id.to_string()),
            ("format", self.audio_format.clone()),
            ("lang", self.lang.clone()),
            ("text", text.to_string()),
        ];
        let resp = http_client()
            .get(format!("{}/voice/vits", self.api_url))
            .query(&query)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("VITS 请求失败: HTTP {}", resp.status()));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("api_url".into(), json!(self.api_url));
        m.insert("speaker_id".into(), json!(self.speaker_id));
        m.insert("audio_format".into(), json!(self.audio_format));
        m.insert("lang".into(), json!(self.lang));
        m
    }
}
