//! Simple-Vits-API Bert-Vits2 适配器，对应 `ling_chat/core/TTS/bv2_adapter.py`。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct Bv2Adapter {
    api_url: String,
    speaker_id: i32,
    audio_format: String,
    lang: String,
}

impl Bv2Adapter {
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
impl TtsAdapter for Bv2Adapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let body = json!({
            "id": self.speaker_id,
            "format": self.audio_format,
            "lang": self.lang,
            "length": 1.0,
            "noise": 0.33,
            "noisew": 0.4,
            "segment_size": 50,
            "sdp_radio": 0.2,
            "text": text,
        });
        let resp = http_client()
            .post(format!("{}/voice/bert-vits2", self.api_url))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("BV2 请求失败: HTTP {status}: {text}"));
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
