//! Window title event — rewrites the OS main-window title for horror staging
//! (e.g. garbled text while the blood UI is up, DDLC-style title corruption).
//!
//! The title is a pure in-memory effect: nothing is persisted, and both the
//! natural end and the manual stop path restore the default title through
//! [`restore_window_title`]. Scripts can also restore early with `title: ''`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::{SCRIPT_WINDOW_TITLE, SCRIPT_WINDOW_TITLE_RESET},
    WindowTitlePayload,
};
use crate::ai_service::message_system::events::emit;
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

/// 通知前端唯一标题协调器清除所有剧本标题意图。
pub fn restore_window_title(app: &tauri::AppHandle) {
    if let Err(error) = emit(app, SCRIPT_WINDOW_TITLE_RESET, &()) {
        tracing::warn!("[WindowTitleEvent] 发送标题重置事件失败: {error:#}");
    }
}

#[async_trait]
impl ScriptEvent for WindowTitleEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 进入前端 FIFO，才能与玩家实际看到的背景特效保持同一时间线。
        emit(
            ctx.app,
            SCRIPT_WINDOW_TITLE,
            &WindowTitlePayload {
                title: self.title.clone(),
            },
        )?;
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
