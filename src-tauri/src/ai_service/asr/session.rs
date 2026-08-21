//! ASR 会话编排：互斥锁 + 取消令牌 + vad / providers 协调。

use std::collections::HashMap;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::error::AsrError;
use super::provider::{AsrProvider, AsrResult};
use super::provider_stream::{self, StreamCommand};
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

/// 流式会话句柄：命令通道 + 注册 provider id。
#[derive(Clone)]
pub struct StreamHandle {
    pub provider_id: String,
    pub tx: tokio::sync::mpsc::UnboundedSender<StreamCommand>,
}

/// ASR 会话编排器。
///
/// - `vad`：共享的 Silero VAD 端点检测器（每次 start 时 reset）。
/// - `providers`：provider id → 实例，注册表。
/// - `active_source`：当前活跃的会话来源（None 表示无活跃会话）。
/// - `cancel_token`：长生命周期取消令牌；cancel 不会立即停掉 in-flight 推理，
///   只让持续轮询的下游（如未来的 hotkey listener loop）有机会退出。
/// - `stream`：流式识别会话句柄（WebSocket 连接常驻后台 task）。
/// - `lock`：互斥锁，保证 start/stop 序列原子化。
pub struct AsrSession {
    pub vad: Arc<AsrVad>,
    pub providers: HashMap<String, Arc<dyn AsrProvider>>,
    pub active_source: Mutex<Option<AsrSource>>,
    pub cancel_token: CancellationToken,
    pub stream: Mutex<Option<StreamHandle>>,
    pub lock: Mutex<()>,
}

impl AsrSession {
    pub fn new(vad: Arc<AsrVad>, providers: HashMap<String, Arc<dyn AsrProvider>>) -> Self {
        Self {
            vad,
            providers,
            active_source: Mutex::new(None),
            cancel_token: CancellationToken::new(),
            stream: Mutex::new(None),
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

    /// 启动流式会话。互斥只查 stream 自身（VAD/active_source 互斥由
    /// 前端的 `asr_start_listening` 负责——两个命令都检查 active_source
    /// 会互相挡住：前端顺序是 start_streaming 先、start_listening 后）。
    pub async fn start_streaming(
        &self,
        app: &tauri::AppHandle,
        provider_id: &str,
        api_key: String,
        model: String,
        language_hint: Option<String>,
    ) -> Result<(), AsrError> {
        // 防御：残留句柄（前端异常路径未清理）先丢弃，避免 SessionBusy
        // 卡死后续所有录音（症状：流式启动失败 → 无法录音）
        if self.stream.lock().await.take().is_some() {
            tracing::warn!("[ASR/stream] 丢弃残留流式会话句柄");
        }
        let tx =
            provider_stream::start_streaming(app.clone(), api_key, model, language_hint).await?;
        *self.stream.lock().await = Some(StreamHandle {
            provider_id: provider_id.to_string(),
            tx,
        });
        Ok(())
    }

    /// 转发 PCM 块到流式连接（不持锁：写循环自身串行）。
    pub async fn stream_audio_chunk(&self, pcm: Vec<f32>) -> Result<(), AsrError> {
        let handle = self.stream.lock().await.clone();
        match handle {
            Some(h) => {
                h.tx.send(StreamCommand::Audio(pcm))
                    .map_err(|_| AsrError::Canceled)
            }
            None => Err(AsrError::EngineLoadFailed("流式会话未启动".into())),
        }
    }

    /// 停止流式会话：发 stop → 等整段 final → 清空句柄 → 返回结果。
    /// （active_source 由前端的 asr_stop_listening 清理，本方法不管。）
    pub async fn stop_streaming(&self) -> Result<AsrResult, AsrError> {
        let handle = self.stream.lock().await.take();
        let Some(h) = handle else {
            return Err(AsrError::Canceled);
        };
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        h.tx.send(StreamCommand::Stop { reply: reply_tx })
            .map_err(|_| AsrError::Canceled)?;
        // 超时保护：服务端不回 result 时不能无限等（前端 stop 会卡死）
        let result = tokio::time::timeout(std::time::Duration::from_secs(30), reply_rx)
            .await
            .map_err(|_| AsrError::ProviderTimeout("qwen-asr".into()))?
            .map_err(|_| AsrError::ProviderTimeout("qwen-asr".into()))?;
        let text = result?.text;
        Ok(AsrResult {
            text,
            language: None,
            confidence: None,
            provider_id: h.provider_id,
        })
    }

    /// 取消流式会话（丢弃连接，不等待 final）。
    pub async fn cancel_stream(&self) {
        let mut g = self.stream.lock().await;
        g.take();
    }
}
