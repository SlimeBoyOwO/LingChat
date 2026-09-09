//! Native system-window event for DDLC-style desktop intrusion staging.
//!
//! The Rust story timeline only emits a queue-ordered frontend event. When the
//! player reaches that exact beat, the Tauri command opens a real Windows
//! TaskDialog, Notepad window, or cmd.exe console without PowerShell/pwsh.
//!
//! Safety contract:
//! - only horror-warning scripts with `allow_system_effects: true` may use it;
//! - title/text stay display-only Unicode with strict total bounds;
//! - styles are a fixed allowlist (`console`, `blood_cmd`, `error`, `warning`,
//!   `notepad`), count is at most four, lifetime at most twelve seconds;
//! - native objects belong to a cancellable run generation and close on stop.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_CONSOLE_WINDOW, ConsoleWindowTicketPayload,
};
use crate::ai_service::message_system::events::emit;

pub(crate) const MAX_WINDOWS: usize = 4;
pub(crate) const MAX_LIFETIME_SECS: u64 = 12;
pub(crate) const MAX_TITLE_CHARS: usize = 80;
pub(crate) const MAX_TEXT_CHARS: usize = 1200;

pub(crate) fn sanitize_console_field(raw: &str) -> String {
    raw.chars()
        .filter(|character| !character.is_control())
        .take(MAX_TITLE_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

pub(crate) fn sanitize_console_text(raw: &str) -> String {
    raw.chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
        .take(MAX_TEXT_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

fn style_is_supported(style: &str) -> bool {
    matches!(
        style,
        "console" | "blood_cmd" | "error" | "warning" | "notepad"
    )
}

pub struct ConsoleWindowEvent {
    title: String,
    text: String,
    count: usize,
    interval: f64,
    lifetime: f64,
    style: String,
}

impl ConsoleWindowEvent {
    fn from_event_data(data: &Value) -> Self {
        let title = sanitize_console_field(
            data.get("title")
                .and_then(|value| value.as_str())
                .unwrap_or("RUNTIME"),
        );
        let text = sanitize_console_text(
            data.get("text")
                .and_then(|value| value.as_str())
                .unwrap_or("..."),
        );
        let count = data
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .clamp(1, MAX_WINDOWS as u64) as usize;
        let raw_interval = data.get("interval").and_then(Value::as_f64).unwrap_or(0.25);
        let interval = if raw_interval.is_finite() {
            raw_interval.clamp(0.0, 5.0)
        } else {
            0.25
        };
        let raw_lifetime = data.get("lifetime").and_then(Value::as_f64).unwrap_or(4.0);
        let lifetime = if raw_lifetime.is_finite() {
            raw_lifetime.clamp(1.0, MAX_LIFETIME_SECS as f64)
        } else {
            4.0
        };
        let style = data
            .get("style")
            .and_then(|value| value.as_str())
            .unwrap_or("console")
            .to_string();
        Self {
            title,
            text,
            count,
            interval,
            lifetime,
            style,
        }
    }

    fn validate(&self) -> Result<()> {
        if !style_is_supported(&self.style) {
            return Err(anyhow!("不支持的 console_window 样式: {}", self.style));
        }
        if self.title.is_empty() {
            return Err(anyhow!("console_window title 不能为空"));
        }
        if self.text.is_empty() {
            return Err(anyhow!("console_window text 不能为空"));
        }
        Ok(())
    }
}

#[async_trait]
impl ScriptEvent for ConsoleWindowEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        self.validate()?;
        // 编辑器试玩不允许把窗口弹到真实桌面。
        if ctx.is_preview {
            return Ok(None);
        }
        let allowed = {
            let game_status = ctx.game_status.lock().await;
            let status = game_status
                .script_status
                .as_ref()
                .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法创建系统弹窗"))?;
            super::glitch_window_event::script_allows_system_effects(status)
        };
        if !allowed {
            return Err(anyhow!(
                "console_window 仅允许 content_warning=horror 且 script_settings.allow_system_effects=true 的剧本使用"
            ));
        }

        let request_id =
            crate::api::script_popups::queue_pending(crate::api::script_popups::PopupSequence {
                title: self.title.clone(),
                lines: self.text.lines().map(ToString::to_string).collect(),
                count: self.count,
                interval: self.interval,
                lifetime: self.lifetime,
                style: self.style.clone(),
            })
            .map_err(anyhow::Error::msg)?;
        tracing::info!(
            "[ConsoleWindowEvent] 票据 {} 已排队（style={} count={}）",
            request_id,
            self.style,
            self.count
        );
        let payload = ConsoleWindowTicketPayload { request_id };
        if let Err(error) = emit(ctx.app, SCRIPT_CONSOLE_WINDOW, &payload) {
            crate::api::script_popups::discard_pending(request_id);
            return Err(error);
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
