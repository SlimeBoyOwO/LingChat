// Bridges the existing `TtsAdapter` trait to the local engine.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::{json, Value as JsonValue};

use crate::ai_service::tts::provider::TtsAdapter;
use super::{LocalTtsEngine, LocalTtsPaths, SynthesizeRequest};

pub struct LocalTtsAdapter {
    engine: Arc<LocalTtsEngine>,
    voice_id: String,
    style_id: i32,
    speaker_id: i64,
    sdp_ratio: f32,
    length_scale: f32,
    paths: LocalTtsPaths,
    ready: AtomicBool,
    bootstrap_lock: tokio::sync::Mutex<()>,
}

impl LocalTtsAdapter {
    #[allow(clippy::too_many_arguments)]
    pub fn with_params(
        engine: Arc<LocalTtsEngine>,
        voice_id: String,
        style_id: i32,
        speaker_id: i64,
        sdp_ratio: f32,
        length_scale: f32,
        paths: LocalTtsPaths,
    ) -> Self {
        Self {
            engine,
            voice_id,
            style_id,
            speaker_id,
            sdp_ratio,
            length_scale,
            paths,
            ready: AtomicBool::new(false),
            bootstrap_lock: tokio::sync::Mutex::new(()),
        }
    }
}

#[async_trait]
impl TtsAdapter for LocalTtsAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        if !self.ready.load(Ordering::Acquire) {
            let _bootstrap_guard = self.bootstrap_lock.lock().await;
            if !self.ready.load(Ordering::Acquire) {
                self.bootstrap().await?;
                self.ready.store(true, Ordering::Release);
            }
        }
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

impl LocalTtsAdapter {
    async fn bootstrap(&self) -> Result<()> {
        if !self.engine.is_ready().await {
            self.engine
                .init(&self.paths)
                .await
                .map_err(|e| anyhow!("local tts init: {e}"))?;
        }
        self.engine
            .load_voice(&self.paths, &self.voice_id)
            .await
            .map_err(|e| anyhow!("local tts load_voice: {e}"))?;
        Ok(())
    }
}
