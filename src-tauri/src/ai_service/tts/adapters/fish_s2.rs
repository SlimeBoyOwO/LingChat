//! Fish Audio S2 HTTP API adapter.
//!
//! Compatible with the s2.cpp server endpoint:
//! `POST /generate` using `multipart/form-data` fields `voice`, `text`, and
//! JSON-encoded `params`.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use reqwest::multipart::Form;
use serde_json::{json, Value as JsonValue};
use tokio::sync::Mutex;

use crate::ai_service::tts::adapters::http_client;
use crate::ai_service::tts::provider::TtsAdapter;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

// s2.cpp currently handles one generation at a time. LingChat synthesizes
// emotion segments concurrently, so serialize all Fish S2 calls process-wide.
static REQUEST_GATE: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

#[derive(Debug, Clone)]
pub struct FishS2Adapter {
    api_url: String,
    voice: String,
}

impl FishS2Adapter {
    pub fn new(api_url: String, voice: String) -> Result<Self> {
        if api_url.trim().is_empty() {
            return Err(anyhow!("Fish S2 API URL is not configured"));
        }
        if voice.trim().is_empty() {
            return Err(anyhow!("Fish S2 voice is not configured"));
        }

        Ok(Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            voice: voice.trim().to_string(),
        })
    }

    fn endpoint(&self) -> String {
        if self.api_url.ends_with("/generate") {
            self.api_url.clone()
        } else {
            format!("{}/generate", self.api_url)
        }
    }
}

fn normalize_emotion(emo: &str) -> Option<&'static str> {
    match emo.trim().to_ascii_lowercase().as_str() {
        "happy" | "happiness" | "joy" | "joyful" | "\u{9ad8}\u{5174}" | "\u{5f00}\u{5fc3}" => {
            Some("happy")
        }
        "excited" | "excitement" | "\u{5174}\u{594b}" => Some("excited"),
        "sad" | "sadness" | "depressed" | "\u{60b2}\u{4f24}" | "\u{4f24}\u{5fc3}" => Some("sad"),
        "angry" | "anger" | "\u{751f}\u{6c14}" => Some("angry"),
        "fear" | "fearful" | "scared" | "\u{5bb3}\u{6015}" => Some("fearful"),
        "surprise" | "surprised" | "\u{60ca}\u{8bb6}" => Some("surprised"),
        "disgust" | "disgusted" | "\u{538c}\u{6076}" => Some("disgusted"),
        "whisper" | "whispering" | "\u{8033}\u{8bed}" | "\u{4f4e}\u{8bed}" => Some("whisper"),
        "cry" | "crying" | "sob" | "sobbing" | "\u{54ed}\u{6ce3}" | "\u{5927}\u{54ed}"
        | "\u{54ed}\u{8154}" => Some("crying"),
        "nervous" | "\u{614c}\u{5f20}" | "\u{7d27}\u{5f20}" => Some("nervous"),
        "worried" | "\u{62c5}\u{5fc3}" => Some("worried"),
        "embarrassed" | "\u{5c34}\u{5c2c}" | "\u{96be}\u{4e3a}\u{60c5}" | "\u{7f9e}\u{803b}" => {
            Some("embarrassed")
        }
        "confident" | "\u{81ea}\u{4fe1}" => Some("confident"),
        "shy" | "\u{5bb3}\u{7f9e}" => Some("shy"),
        "serious" | "\u{8ba4}\u{771f}" | "\u{6b63}\u{7ecf}" => Some("serious"),
        "confused" | "\u{7591}\u{60d1}" => Some("confused"),
        "affectionate" | "\u{60c5}\u{52a8}" | "\u{5fc3}\u{52a8}" => Some("affectionate"),
        "playful" | "\u{8c03}\u{76ae}" => Some("playful"),
        "calm" | "\u{5e73}\u{9759}" => Some("calm"),
        "gentle" | "\u{6e29}\u{67d4}" => Some("gentle"),
        _ => None,
    }
}

fn text_with_emotion(text: &str, emo: &str) -> String {
    let text = text.trim();
    match normalize_emotion(emo) {
        Some(tag) if !text.starts_with('[') => format!("[{tag}]{text}"),
        _ => text.to_string(),
    }
}

#[async_trait]
impl TtsAdapter for FishS2Adapter {
    async fn generate_voice(&self, text: &str, emo: &str) -> Result<Vec<u8>> {
        if text.trim().is_empty() {
            return Err(anyhow!("Fish S2 input text is empty"));
        }

        let _request_guard = REQUEST_GATE.lock().await;

        let params = json!({
            "max_new_tokens": 512,
            "temperature": 0.8,
            "top_p": 0.9,
            "top_k": 40
        });
        let synthesized_text = text_with_emotion(text, emo);
        let form = Form::new()
            .text("voice", self.voice.clone())
            .text("text", synthesized_text)
            .text("params", params.to_string());

        let response = http_client()
            .post(self.endpoint())
            .timeout(REQUEST_TIMEOUT)
            .multipart(form)
            .send()
            .await
            .context(
                "Failed to connect to Fish S2; ensure the local API is listening on port 3030",
            )?;

        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("Failed to read Fish S2 response")?;
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(anyhow!("Fish S2 request failed: HTTP {status}: {body}"));
        }
        if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
            return Err(anyhow!("Fish S2 response is not a valid WAV file"));
        }

        Ok(bytes.to_vec())
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        HashMap::from([
            ("api_url".into(), json!(self.api_url)),
            ("voice".into(), json!(self.voice)),
        ])
    }
}
