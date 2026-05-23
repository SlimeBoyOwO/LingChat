//! AIVIS 云 API 适配器，对应 `ling_chat/core/TTS/aivis_adapter.py`。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Map, Value, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct AivisAdapter {
    api_url: String,
    api_key: String,
    model_uuid: String,
    speaker_uuid: Option<String>,
    style_id: Option<i32>,
    style_name: Option<String>,
    audio_format: String,
    lang: String,
}

impl AivisAdapter {
    pub fn new(
        api_url: String,
        api_key: Option<String>,
        model_uuid: String,
        speaker_uuid: Option<String>,
        audio_format: String,
        lang: String,
    ) -> Result<Self> {
        let api_url = api_url.trim_end_matches('/').to_string();
        let api_key = api_key
            .filter(|k| !k.is_empty())
            .ok_or_else(|| anyhow!("未设置 AIVIS API 密钥（请在设置中配置 tts.aivis_api_key）"))?;
        Ok(Self {
            api_url,
            api_key,
            model_uuid,
            speaker_uuid,
            style_id: None,
            style_name: None,
            audio_format,
            lang,
        })
    }
}

#[async_trait]
impl TtsAdapter for AivisAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let accept = match self.audio_format.as_str() {
            "wav" => "audio/wav",
            "flac" => "audio/flac",
            "mp3" => "audio/mpeg",
            "aac" => "audio/aac",
            "opus" => "audio/ogg; codecs=opus",
            _ => "audio/mpeg",
        };

        let mut params: Map<String, Value> = Map::new();
        params.insert("model_uuid".into(), json!(self.model_uuid));
        if let Some(v) = &self.speaker_uuid {
            params.insert("speaker_uuid".into(), json!(v));
        }
        if let Some(v) = self.style_id {
            params.insert("style_id".into(), json!(v));
        }
        if let Some(v) = &self.style_name {
            params.insert("style_name".into(), json!(v));
        }
        params.insert("language".into(), json!(self.lang));
        params.insert("use_ssml".into(), json!(true));
        params.insert("speaking_rate".into(), json!(1.0));
        params.insert("emotional_intensity".into(), json!(1.0));
        params.insert("tempo_dynamics".into(), json!(1.0));
        params.insert("pitch".into(), json!(0.0));
        params.insert("volume".into(), json!(1.0));
        params.insert("leading_silence_seconds".into(), json!(0.1));
        params.insert("trailing_silence_seconds".into(), json!(0.1));
        params.insert("line_break_silence_seconds".into(), json!(0.4));
        params.insert("output_format".into(), json!(self.audio_format));
        params.insert("output_sampling_rate".into(), json!(44100));
        params.insert("output_audio_channels".into(), json!("mono"));
        params.insert("text".into(), json!(text));

        let resp = http_client()
            .post(format!("{}/tts/synthesize", self.api_url))
            .header(reqwest::header::ACCEPT, accept)
            .bearer_auth(&self.api_key)
            .json(&Value::Object(params))
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("AIVIS 请求失败: HTTP {status}: {text}"));
        }
        Ok(resp.bytes().await?.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("api_url".into(), json!(self.api_url));
        m.insert("model_uuid".into(), json!(self.model_uuid));
        m.insert("speaker_uuid".into(), json!(self.speaker_uuid));
        m.insert("audio_format".into(), json!(self.audio_format));
        m.insert("lang".into(), json!(self.lang));
        m
    }
}
