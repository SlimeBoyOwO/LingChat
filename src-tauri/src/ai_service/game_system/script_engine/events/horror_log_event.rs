//! Horror log event — floods the log window with blood-red ERROR lines
//! (the log console renders error level in red). Pure backend effect:
//! no frontend event is emitted, the lines travel via the log bridge.
//!
//! YAML: `- type: horror_log` with `text` (message) and `lines` (repeat
//! count, default 1, clamped to 50 to keep the 5000-line buffer sane).

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};

const MAX_LINES: i64 = 50;
/// 乱码碎片：追加在文本后面，模拟"日志本身被污染"的感觉
const GLITCH_SHARDS: [&str; 8] = [
    "▓▒░", "█▓▒", "░▒▓", "▒█░", "▓░█", "█░▒", "░█▓", "▒░▓",
];

pub struct HorrorLogEvent {
    text: String,
    lines: i64,
    duration: Option<f64>,
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
        Self {
            text,
            lines,
            duration: parse_duration(data),
        }
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

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(HorrorLogEvent::event_type(), |data| {
        Box::new(HorrorLogEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::HorrorLogEvent;
    use serde_json::json;

    #[test]
    fn defaults() {
        let e = HorrorLogEvent::from_event_data(&json!({}));
        assert_eq!(e.text, "SHE_SEES_YOU");
        assert_eq!(e.lines, 1);
    }

    #[test]
    fn lines_clamped() {
        assert_eq!(HorrorLogEvent::from_event_data(&json!({"lines": 0})).lines, 1);
        assert_eq!(HorrorLogEvent::from_event_data(&json!({"lines": 999})).lines, 50);
        assert_eq!(HorrorLogEvent::from_event_data(&json!({"lines": 12})).lines, 12);
    }
}
