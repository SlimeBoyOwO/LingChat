// Bridges the existing `TtsAdapter` trait to the local engine.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
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
    /// bootstrap 完成时记录的引擎卸载版本（见 [`LocalTtsEngine::version`]）。
    /// 引擎每次 `unload_all`（设备热切换、TTS 关闭）递增版本；不一致说明
    /// 声线已被卸载，需重新 bootstrap 加载。
    bootstrapped_version: AtomicU64,
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
            bootstrapped_version: AtomicU64::new(0),
            bootstrap_lock: tokio::sync::Mutex::new(()),
        }
    }
}

#[async_trait]
impl TtsAdapter for LocalTtsAdapter {
    async fn generate_voice(&self, text: &str, _emo: &str) -> Result<Vec<u8>> {
        self.ensure_ready().await?;
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
    /// 确保引擎与当前声线已按最新设备加载。
    /// 快速路径只在"已就绪且引擎卸载版本未变"时跳过；设备热切换
    /// （`unload_all` 递增版本）或 TTS 关闭重开后会重新 bootstrap。
    async fn ensure_ready(&self) -> Result<()> {
        if self.ready.load(Ordering::Acquire)
            && self.bootstrapped_version.load(Ordering::Acquire) == self.engine.version()
        {
            return Ok(());
        }

        let _bootstrap_guard = self.bootstrap_lock.lock().await;
        // bootstrap 期间设备切换可能再次发生（版本继续递增），此时本次加载的
        // 声线已被清空；只有加载完成后版本未变才把结果记录为有效，否则重试。
        loop {
            if self.ready.load(Ordering::Acquire)
                && self.bootstrapped_version.load(Ordering::Acquire) == self.engine.version()
            {
                return Ok(());
            }

            let version_before = self.engine.version();
            self.bootstrap().await?;
            if version_before == self.engine.version() {
                self.bootstrapped_version
                    .store(version_before, Ordering::Release);
                self.ready.store(true, Ordering::Release);
                return Ok(());
            }
        }
    }

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
