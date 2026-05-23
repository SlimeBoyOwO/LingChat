//! Style-Bert-Vits2 本地服务适配器，对应 `ling_chat/core/TTS/sbv2_adapter.py`。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct Sbv2Adapter {
    api_url: String,
    audio_format: String,
    model_name: String,
    speaker_id: i32,
    language: String,
}

impl Sbv2Adapter {
    pub fn new(api_url: String, speaker_id: i32, model_name: String, audio_format: String, lang: &str) -> Self {
        let language = if lang == "ja" { "JP".to_string() } else { lang.to_string() };
        let api_url = api_url.trim_end_matches('/').to_string();
        Self {
            api_url,
            audio_format,
            model_name,
            speaker_id,
            language,
        }
    }
}

#[async_trait]
impl TtsAdapter for Sbv2Adapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let accept = match self.audio_format.as_str() {
            "wav" => "audio/wav",
            "flac" => "audio/flac",
            "mp3" => "audio/mpeg",
            "aac" => "audio/aac",
            "ogg" => "audio/ogg",
            _ => "audio/wav",
        };

        let query: Vec<(&str, String)> = vec![
            ("encoding", "utf-8".to_string()),
            ("model_name", self.model_name.clone()),
            ("model_id", "0".to_string()),
            ("speaker_id", self.speaker_id.to_string()),
            ("sdp_ratio", "0.2".to_string()),
            ("noise", "0.6".to_string()),
            ("noisew", "0.8".to_string()),
            ("length", "1.0".to_string()),
            ("language", self.language.clone()),
            ("split_interval", "0.5".to_string()),
            ("style", "Neutral".to_string()),
            ("style_weight", "1.0".to_string()),
            ("text", text.to_string()),
        ];

        let resp = http_client()
            .post(format!("{}/voice", self.api_url))
            .query(&query)
            .header(reqwest::header::ACCEPT, accept)
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("SBV2 请求失败: HTTP {}", resp.status()));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("api_url".into(), json!(self.api_url));
        m.insert("audio_format".into(), json!(self.audio_format));
        m.insert("model_name".into(), json!(self.model_name));
        m.insert("speaker_id".into(), json!(self.speaker_id));
        m.insert("language".into(), json!(self.language));
        m
    }
}
