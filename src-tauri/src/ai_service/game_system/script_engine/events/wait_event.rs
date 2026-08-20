//! Wait event — holds the script timeline for N seconds.
//!
//! DDLC 式定拍演出（发现 CG 后静止 3.75s、假报错挂 6s 等）靠它撑起来：
//! 非交互事件默认瞬时连发，没有时间轴上的"停在这一拍"手段。
//! 只阻塞剧本时间轴，不向前端发任何消息。

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};

/// 单次等待上限，防止笔误把剧本卡死
const MAX_WAIT_SECS: f64 = 30.0;

pub struct WaitEvent {
    seconds: f64,
}

impl WaitEvent {
    fn from_event_data(data: &Value) -> Self {
        let seconds = data
            .get("seconds")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        Self { seconds }
    }
}

#[async_trait]
impl ScriptEvent for WaitEvent {
    async fn execute(&mut self, _ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let secs = self.seconds.clamp(0.05, MAX_WAIT_SECS);
        if self.seconds > MAX_WAIT_SECS {
            tracing::warn!(
                "[WaitEvent] seconds={} 超过上限，已截断为 {}s",
                self.seconds,
                MAX_WAIT_SECS
            );
        }
        tokio::time::sleep(std::time::Duration::from_secs_f64(secs)).await;
        Ok(None)
    }

    fn event_type() -> &'static str {
        "wait"
    }
}

pub fn register() {
    register_event(WaitEvent::event_type(), |data| {
        Box::new(WaitEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::WaitEvent;
    use serde_json::json;

    #[test]
    fn parses_seconds() {
        let e = WaitEvent::from_event_data(&json!({ "seconds": 3.75 }));
        assert_eq!(e.seconds, 3.75);
    }

    #[test]
    fn defaults_to_one_second() {
        let e = WaitEvent::from_event_data(&json!({}));
        assert_eq!(e.seconds, 1.0);
    }
}
