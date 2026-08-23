//! Window title event — rewrites the OS main-window title for horror staging
//! (e.g. garbled text while the blood UI is up, DDLC-style title corruption).
//!
//! The title is a pure in-memory effect: nothing is persisted, and both the
//! natural end and the manual stop path restore the default title through
//! [`restore_window_title`]. Scripts can also restore early with `title: ''`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};

/// 与 tauri.conf.json 的窗口 title 保持一致
const DEFAULT_TITLE: &str = "LingChat";
/// 乱码标题不需要很长；过长标题在某些平台会被截断甚至撑破任务栏预览
const MAX_TITLE_CHARS: usize = 80;

pub struct WindowTitleEvent {
    title: String,
}

impl WindowTitleEvent {
    fn from_event_data(data: &Value) -> Self {
        let raw = data
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();
        // 控制字符会让标题栏渲染出不可预测的占位符，直接剥掉
        let sanitized: String = raw.chars().filter(|c| !c.is_control()).collect();
        let truncated: String = sanitized.chars().take(MAX_TITLE_CHARS).collect();
        Self { title: truncated }
    }
}

/// 恢复默认窗口标题。剧本自然结束与手动停止都会经过这里。
pub fn restore_window_title(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(error) = window.set_title(DEFAULT_TITLE) {
            tracing::warn!("[WindowTitleEvent] 恢复窗口标题失败: {error}");
        }
    }
}

#[async_trait]
impl ScriptEvent for WindowTitleEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let title = if self.title.is_empty() {
            DEFAULT_TITLE
        } else {
            self.title.as_str()
        };
        if let Some(window) = ctx.app.get_webview_window("main") {
            if let Err(error) = window.set_title(title) {
                // 标题写不进去不该中断剧本
                tracing::warn!("[WindowTitleEvent] 设置窗口标题失败: {error}");
            }
        }
        Ok(None)
    }

    fn event_type() -> &'static str {
        "window_title"
    }
}

pub fn register() {
    register_event(WindowTitleEvent::event_type(), |data| {
        Box::new(WindowTitleEvent::from_event_data(&data))
    });
}
