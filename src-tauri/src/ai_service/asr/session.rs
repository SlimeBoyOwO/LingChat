//! ASR 会话编排：互斥锁 + 取消令牌 + vad / providers 协调。

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::error::AsrError;
use super::provider::{AsrProvider, AsrResult};
use super::vad::AsrVad;

/// ASR 会话来源。三种触发源共享同一会话生命周期。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrSource {
    Button,
    Hotkey,
    Auto,
}

impl AsrSource {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "button" => Some(Self::Button),
            "hotkey" => Some(Self::Hotkey),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// ASR 会话编排器。
///
/// - `vad`：共享的 Silero VAD 端点检测器（每次 start 时 reset）。
/// - `providers`：provider id → 实例，注册表。
/// - `active_source`：当前活跃的会话来源（None 表示无活跃会话）。
/// - `cancel_token`：长生命周期取消令牌；cancel 不会立即停掉 in-flight 推理，
///   只让持续轮询的下游（如未来的 hotkey listener loop）有机会退出。
/// - `lock`：互斥锁，保证 start/stop 序列原子化。
pub struct AsrSession {
    pub vad: Arc<AsrVad>,
    pub providers: HashMap<String, Arc<dyn AsrProvider>>,
    pub active_source: Mutex<Option<AsrSource>>,
    pub cancel_token: CancellationToken,
    pub lock: Mutex<()>,
}

impl AsrSession {
    pub fn new(vad: Arc<AsrVad>, providers: HashMap<String, Arc<dyn AsrProvider>>) -> Self {
        Self {
            vad,
            providers,
            active_source: Mutex::new(None),
            cancel_token: CancellationToken::new(),
            lock: Mutex::new(()),
        }
    }

    /// 启动一个 ASR 会话。互斥：已有活跃会话则返回 SessionBusy。
    pub async fn start(&self, source: AsrSource) -> Result<(), AsrError> {
        let _guard = self.lock.lock().await;
        let mut active = self.active_source.lock().await;
        if active.is_some() {
            return Err(AsrError::SessionBusy);
        }
        *active = Some(source);
        self.vad.reset().await;
        Ok(())
    }

    /// 停止指定 source 的会话。source 不匹配返回 Canceled（视为取消）。
    pub async fn stop(&self, source: AsrSource) -> Result<(), AsrError> {
        let mut active = self.active_source.lock().await;
        if *active != Some(source) {
            return Err(AsrError::Canceled);
        }
        *active = None;
        Ok(())
    }

    /// 转发 30ms PCM 块到 VAD（session 未活跃也允许：前端可能误调）。
    pub async fn vad_process_chunk(
        &self,
        app: &tauri::AppHandle,
        pcm: Vec<f32>,
    ) -> Result<(), AsrError> {
        self.vad.process_chunk(app, &pcm).await.map(|_| ())
    }

    /// 调用指定 provider 识别一段 WAV 字节。
    pub async fn recognize_wav(
        &self,
        provider_id: String,
        wav_bytes: Vec<u8>,
        language_hint: Option<String>,
    ) -> Result<AsrResult, AsrError> {
        let provider = self
            .providers
            .get(&provider_id)
            .ok_or_else(|| AsrError::ProviderNotFound(provider_id.clone()))?;
        self.recognize_wav_with(provider.clone(), wav_bytes, language_hint.as_deref())
            .await
    }

    /// 识别 + 取消支持：`tokio::select!` 竞争 provider 结果与取消令牌。
    /// child_token 每次新建 —— 上一次 `cancel()` 不会影响后续识别。
    pub async fn recognize_wav_with(
        &self,
        provider: Arc<dyn AsrProvider>,
        wav_bytes: Vec<u8>,
        language_hint: Option<&str>,
    ) -> Result<AsrResult, AsrError> {
        let cancel_child = self.cancel_token.child_token();
        tokio::select! {
            result = provider.recognize(wav_bytes, language_hint) => result,
            _ = cancel_child.cancelled() => Err(AsrError::Canceled),
        }
    }

    pub async fn current_source(&self) -> Option<AsrSource> {
        *self.active_source.lock().await
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }
}