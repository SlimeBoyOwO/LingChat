// Bridges the existing `TtsAdapter` trait to the new local engine.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use super::engine::{LocalTtsEngine, SynthesizeRequest};
use crate::ai_service::tts::provider::TtsAdapter;

pub struct LocalTtsAdapter {
    engine: Arc<LocalTtsEngine>,
    voice_id: String,
    speaker_id: i64,
    style_id: i32,
    length_scale: f32,
    sdp_ratio: f32,
}

impl LocalTtsAdapter {
    pub fn new(
        engine: Arc<LocalTtsEngine>,
        voice_id: String,
        speaker_id: i64,
    ) -> Self {
        Self {
            engine,
            voice_id,
            speaker_id,
            style_id: 0,
            length_scale: 1.0,
            sdp_ratio: 0.0,
        }
    }

    pub fn with_params(
        engine: Arc<LocalTtsEngine>,
        voice_id: String,
        speaker_id: i64,
        style_id: i32,
        length_scale: f32,
        sdp_ratio: f32,
    ) -> Self {
        Self {
            engine,
            voice_id,
            speaker_id,
            style_id,
            length_scale,
            sdp_ratio,
        }
    }
}

#[async_trait]
impl TtsAdapter for LocalTtsAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        let req = SynthesizeRequest {
            voice_id: self.voice_id.clone(),
            text: text.to_string(),
            style_id: self.style_id,
            speaker_id: self.speaker_id,
            sdp_ratio: self.sdp_ratio,
            length_scale: self.length_scale,
        };
        self.engine.synthesize(req).await.map_err(|e| anyhow!(e))
    }

    fn get_params(&self) -> HashMap<String, JsonValue> {
        let mut m = HashMap::new();
        m.insert("voice_id".into(), json!(self.voice_id));
        m.insert("speaker_id".into(), json!(self.speaker_id));
        m.insert("style_id".into(), json!(self.style_id));
        m.insert("length_scale".into(), json!(self.length_scale));
        m.insert("sdp_ratio".into(), json!(self.sdp_ratio));
        m
    }
}
