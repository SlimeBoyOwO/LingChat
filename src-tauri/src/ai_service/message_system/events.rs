//! 前端事件分发抽象。
//!
//! 旧版基于 WebSocket + `message_broker.publish(client_id, data)`。
//! 现在走 Tauri 的 `Emitter::emit`，把结构化 payload 作为事件分发给前端。
//!
//! 通过 trait 解耦，便于测试和未来加入多目标（多窗口）分发。

use anyhow::Result;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// 前端事件发射器。实现方可以是真正的 AppHandle，也可以是测试 stub。
pub trait EventSink: Send + Sync {
    fn emit_event(&self, event: &str, payload: &dyn erased_payload::ErasedSerialize) -> Result<()>;
}

/// tauri 的 AppHandle 实现 EventSink。
pub struct TauriEventSink {
    pub app: AppHandle,
}

impl TauriEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriEventSink {
    fn emit_event(&self, event: &str, payload: &dyn erased_payload::ErasedSerialize) -> Result<()> {
        let value = payload.to_json_value()?;
        self.app.emit(event, value).map_err(anyhow::Error::from)
    }
}

/// 便捷函数：直接向 AppHandle 发 serde 可序列化 payload。业务层也可以绕过 trait。
pub fn emit<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) -> Result<()> {
    app.emit(event, payload.clone())
        .map_err(anyhow::Error::from)
}

/// 通知前端 AI 正在思考或结束思考。
pub fn emit_thinking(app: &AppHandle, is_thinking: bool) {
    let payload = super::responses::ThinkingResponse::new(is_thinking);
    if let Err(e) = app.emit(super::responses::event_names::AI_THINKING, &payload) {
        tracing::warn!("emit thinking 失败: {e}");
    }
}

/// 通知前端 AI 发生错误，同时重置前端状态为 input。
pub fn emit_error(app: &AppHandle, err: &anyhow::Error) {
    let msg = err.to_string();
    let code = classify_error(&msg);
    let err_payload = super::responses::ErrorResponse::new(code, &msg);
    let _ = app.emit(super::responses::event_names::AI_ERROR, &err_payload);
    let reset = super::responses::StatusResetResponse::new("input");
    let _ = app.emit(super::responses::event_names::STATUS_RESET, &reset);
}

/// 根据错误信息字符串，为前端分类错误类型。
fn classify_error(msg: &str) -> &'static str {
    let lc = msg.to_lowercase();
    if msg.contains("401") || msg.contains("Api key is invalid") {
        "401"
    } else if msg.contains("404") {
        "404"
    } else if lc.contains("network") || msg.contains("网络") {
        "network_error"
    } else {
        "default_error"
    }
}

pub mod erased_payload {
    use anyhow::Result;
    use serde::Serialize;
    use serde_json::Value;

    /// 对 `Serialize` 的对象安全封装，允许通过 `&dyn` 传递。
    pub trait ErasedSerialize {
        fn to_json_value(&self) -> Result<Value>;
    }

    impl<T> ErasedSerialize for T
    where
        T: Serialize,
    {
        fn to_json_value(&self) -> Result<Value> {
            serde_json::to_value(self).map_err(anyhow::Error::from)
        }
    }
}
