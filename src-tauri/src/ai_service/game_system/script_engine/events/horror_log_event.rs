//! Horror log event — floods the log window with blood-red ERROR lines
//! (the log console renders error level in red). Pure backend effect:
//! no frontend event is emitted, the lines travel via the log bridge.
//!
//! YAML: `- type: horror_log` with `text` (message) and `lines` (repeat
//! count, default 1, clamped to 300 so one bounded collapse beat can flood the 5000-line buffer).

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};

const MAX_LINES: i64 = 300;
/// 乱码碎片：追加在文本后面，模拟"日志本身被污染"的感觉
const GLITCH_SHARDS: [&str; 8] = [
    "▓▒░", "█▓▒", "░▒▓", "▒█░", "▓░█", "█░▒", "░█▓", "▒░▓",
];

pub struct HorrorLogEvent {
    text: String,
    lines: i64,
}

impl HorrorLogEvent {
    fn from_event_data(data: &Value) -> Self {
        let text = data
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("SHE_SEES_YOU")
            .to_string();
        let raw = data.get("lines").and_then(|v| v.as_i64()).unwrap_or(1);
        let lines = raw.clamp(1, MAX_LINES);
        if raw != lines {
            tracing::warn!(
                "[HorrorLogEvent] lines {} 超出范围，已截断为 {}（1-{}）",
                raw,
                lines,
                MAX_LINES
            );
        }
        Self { text, lines }
    }
}

#[async_trait]
impl ScriptEvent for HorrorLogEvent {
    async fn execute(&mut self, _ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        for i in 0..self.lines {
            let shard = GLITCH_SHARDS[(i as usize) % GLITCH_SHARDS.len()];
            // error! 级别：日志窗口渲染为血红色，刷屏即"血字刷屏"
            tracing::error!(target: "script_horror", "{} {}", self.text, shard);
        }
        Ok(None)
    }

    fn event_type() -> &'static str {
        "horror_log"
    }
}

pub fn register() {
    register_event(HorrorLogEvent::event_type(), |data| {
        Box::new(HorrorLogEvent::from_event_data(&data))
    });
}
