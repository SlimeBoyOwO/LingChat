//! OpenTTS / OpenAI TTS API 适配器。
//!
//! 参考 AstrBot 的 `astrbot_plugin_tts_tools`：使用 OpenAI 兼容的 `/v1/audio/speech`
//! 端点，默认指向硅基流动 API，默认模型 FunAudioLLM/CosyVoice2-0.5B。

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

#[derive(Debug, Clone)]
pub struct OpenTtsAdapter {
    api_url: String,
    api_key: String,
    model: String,
    voice: String,
    response_format: String,
    language: String,
}

impl OpenTtsAdapter {
    pub fn new(
        api_url: String,
        api_key: String,
        model: String,
        voice: String,
        response_format: String,
        language: String,
    ) -> Result<Self> {
        if api_key.trim().is_empty() {
            return Err(anyhow!("未设置 OpenTTS API 密钥"));
        }
        if voice.trim().is_empty() {
            return Err(anyhow!("未设置 OpenTTS voice"));
        }
        Ok(Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            api_key,
            model,
            voice,
            response_format,
            language,
        })
    }
}

#[async_trait]
impl TtsAdapter for OpenTtsAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            return Err(anyhow!("OpenTTS 输入文本为空"));
        }

        // 硅基流动 / OpenAI 兼容 TTS 端点通常根据输入文本自动识别语言，
        // 不需要额外的 [JA]/[ZH] 控制标记；直接发送原始文本最兼容。
        tracing::debug!("OpenTTS synthesize lang={} text={}", self.language, text);

        let body = SpeechRequest {
            model: &self.model,
            input: text,
            voice: &self.voice,
            response_format: &self.response_format,
        };

        let base_url = self
            .api_url
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .trim_end_matches('/');

        let resp = http_client()
            .post(format!("{}/v1/audio/speech", base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("OpenTTS 请求失败: HTTP {status}: {text}"));
        }

        Ok(resp.bytes().await?.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("api_url".into(), json!(self.api_url));
        m.insert("model".into(), json!(self.model));
        m.insert("voice".into(), json!(self.voice));
        m.insert("response_format".into(), json!(self.response_format));
        m.insert("language".into(), json!(self.language));
        m
    }
}

#[derive(Serialize)]
struct SpeechRequest<'a> {
    model: &'a str,
    input: &'a str,
    voice: &'a str,
    response_format: &'a str,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SpeechError {
    error: serde_json::Value,
}
