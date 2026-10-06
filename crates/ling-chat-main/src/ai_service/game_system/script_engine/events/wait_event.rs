//! Wait event — holds the player-visible frontend timeline for N seconds.
//!
//! DDLC 式定拍演出（发现 CG 后静止 3.75s、假报错挂 6s 等）必须进入
//! 前端事件队列：Rust 端若只 sleep，等待会在玩家仍阅读上一句对白时提前耗尽，
//! 等画面事件真正被消费时便会瞬间连发。

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_WAIT, WaitPayload,
};
use crate::ai_service::message_system::events::emit;

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
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let secs = self.seconds.clamp(0.05, MAX_WAIT_SECS);
        if self.seconds > MAX_WAIT_SECS {
            tracing::warn!(
                "[WaitEvent] seconds={} 超过上限，已截断为 {}s",
                self.seconds,
                MAX_WAIT_SECS
            );
        }
        emit(ctx.app, SCRIPT_WAIT, &WaitPayload { duration: secs })?;
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
