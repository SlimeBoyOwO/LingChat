//! Console window event — emits a frontend-synced ticket so a REAL system
//! console (cmd.exe) appears exactly when the player reaches the beat, for
//! DDLC-style "the story leaks onto your desktop" staging.
//!
//! 为什么不直接在后端 spawn：后端事件循环会一口气跑完非阻塞事件，真实控制台
//! 会提前好几章炸出来。所以这里只发 `script:console-window`，由前端队列在
//! 玩家推进到该拍时调用 `spawn_script_console_window` 命令真正拉起。
//!
//! Safety contract (intentionally narrow):
//! - Only `title` / `text` free text fields; rendered into a fixed temporary
//!   .bat (chcp → title → echo… → ping delay → self-delete) at spawn time.
//! - Shell metacharacters (& | < > % ^ " ` !) and control chars are stripped
//!   from every field — both here and again in the spawn command.
//! - `count` ≤ 4, `lifetime` ≤ 12s; each console auto-closes and self-deletes.
//! - Only usable by horror-warning scripts that explicitly allow system effects
//!   (same gate as glitch_window, enforced by the content validator).

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_CONSOLE_WINDOW, ConsoleWindowPayload,
};
use crate::ai_service::message_system::events::emit;

pub(crate) const MAX_WINDOWS: usize = 4;
pub(crate) const MAX_LIFETIME_SECS: u64 = 12;
pub(crate) const MAX_FIELD_CHARS: usize = 120;

/// 剥掉 cmd 元字符与控制字符：文本只能作为纯字面量进入固定模板
pub(crate) fn sanitize_console_field(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control() && !matches!(c, '&' | '|' | '<' | '>' | '%' | '^' | '"' | '`' | '!'))
        .take(MAX_FIELD_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

pub struct ConsoleWindowEvent {
    title: String,
    text: String,
    count: usize,
    interval: f64,
    lifetime: u64,
    style: String,
}

impl ConsoleWindowEvent {
    fn from_event_data(data: &Value) -> Self {
        let title = sanitize_console_field(
            data.get("title").and_then(|v| v.as_str()).unwrap_or("RUNTIME"),
        );
        let text = data
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("...")
            .to_string();
        let count = data
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .clamp(1, MAX_WINDOWS as u64) as usize;
        let interval = data
            .get("interval")
            .and_then(Value::as_f64)
            .unwrap_or(0.25)
            .clamp(0.0, 5.0);
        let lifetime = data
            .get("lifetime")
            .and_then(Value::as_f64)
            .unwrap_or(4.0)
            .clamp(1.0, MAX_LIFETIME_SECS as f64) as u64;
        let requested = data
            .get("style")
            .and_then(|v| v.as_str())
            .unwrap_or("console");
        let style = match requested {
            "console" | "error" | "warning" | "notepad" => requested.to_string(),
            other => {
                tracing::warn!("[ConsoleWindowEvent] 未知窗口样式 '{}'，回退为 console", other);
                "console".to_string()
            }
        };
        Self {
            title,
            text,
            count,
            interval,
            lifetime,
            style,
        }
    }
}

#[async_trait]
impl ScriptEvent for ConsoleWindowEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 试玩（编辑器预览）不拉起真实系统窗口
        if ctx.is_preview {
            return Ok(None);
        }
        let payload = ConsoleWindowPayload {
            title: self.title.clone(),
            text: self.text.clone(),
            count: self.count,
            interval: self.interval,
            lifetime: self.lifetime,
            style: self.style.clone(),
        };
        if let Err(error) = emit(ctx.app, SCRIPT_CONSOLE_WINDOW, &payload) {
            tracing::warn!("[ConsoleWindowEvent] 发送控制台窗口事件失败（剧本继续）: {error:#}");
        }
        Ok(None)
    }

    fn event_type() -> &'static str {
        "console_window"
    }
}

pub fn register() {
    register_event(ConsoleWindowEvent::event_type(), |data| {
        Box::new(ConsoleWindowEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::ConsoleWindowEvent;
    use serde_json::json;

    #[test]
    fn metacharacters_are_stripped_from_title() {
        let event = ConsoleWindowEvent::from_event_data(&json!({
            "title": "A & del /q",
            "text": "line1 & format c:",
        }));
        assert_eq!(event.title, "A  del /q");
        assert_eq!(event.count, 1);
    }

    #[test]
    fn count_and_lifetime_are_capped() {
        let event = ConsoleWindowEvent::from_event_data(&json!({
            "count": 99,
            "lifetime": 999.0,
        }));
        assert_eq!(event.count, 4);
        assert_eq!(event.lifetime, 12);
    }
}
