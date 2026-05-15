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
    fn emit_event(
        &self,
        event: &str,
        payload: &dyn erased_payload::ErasedSerialize,
    ) -> Result<()> {
        let value = payload.to_json_value()?;
        self.app.emit(event, value).map_err(anyhow::Error::from)
    }
}

/// 便捷函数：直接向 AppHandle 发 serde 可序列化 payload。业务层也可以绕过 trait。
pub fn emit<T: Serialize + Clone>(app: &AppHandle, event: &str, payload: &T) -> Result<()> {
    app.emit(event, payload.clone()).map_err(anyhow::Error::from)
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
