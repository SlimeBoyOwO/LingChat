//! SBV2 API 适配器，对应 `ling_chat/core/TTS/sbv2api_adapter.py`。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct Sbv2ApiAdapter {
    api_url: String,
    model_name: String,
    speaker_id: i32,
    style_id: i32,
    length_scale: f32,
    sdp_ratio: f32,
}

impl Sbv2ApiAdapter {
    pub fn new(api_url: String, model_name: String, speaker_id: i32) -> Self {
        let api_url = api_url.trim_end_matches('/').to_string();
        Self {
            api_url,
            model_name,
            speaker_id,
            style_id: 0,
            length_scale: 1.0,
            sdp_ratio: 0.0,
        }
    }
}

#[async_trait]
impl TtsAdapter for Sbv2ApiAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let body = json!({
            "ident": self.model_name,
            "length_scale": self.length_scale,
            "sdp_ratio": self.sdp_ratio,
            "speaker_id": self.speaker_id,
            "style_id": self.style_id,
            "text": text,
        });
        let resp = http_client()
            .post(format!("{}/synthesize", self.api_url))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("SBV2API 请求失败: HTTP {status}: {text}"));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("api_url".into(), json!(self.api_url));
        m.insert("model_name".into(), json!(self.model_name));
        m.insert("speaker_id".into(), json!(self.speaker_id));
        m.insert("style_id".into(), json!(self.style_id));
        m.insert("length_scale".into(), json!(self.length_scale));
        m.insert("sdp_ratio".into(), json!(self.sdp_ratio));
        m
    }
}
