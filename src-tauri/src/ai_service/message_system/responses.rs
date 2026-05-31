//! 前端消息响应模型。与旧版 `ling_chat.core.schemas.responses` 对应。
//!
//! Tauri 版通过 `window.emit("ai:reply", payload)` 发送给前端。序列化字段
//! 保持与旧版一致（camelCase），使前端事件 handler 可以直接复用。

use serde::{Deserialize, Serialize};

// ============================================================
// 事件名（供 events.rs 使用）
// ============================================================

pub mod event_names {
    /// AI 回复流（每个句子一个事件，最后一个 `isFinal=true`）。
    pub const AI_REPLY: &str = "ai:reply";
    /// AI 思考状态切换。
    pub const AI_THINKING: &str = "ai:thinking";
    /// AI 侧错误（鉴权失败 / 网络错误等）。
    pub const AI_ERROR: &str = "ai:error";
    /// 强制将前端状态重置为 `input`。
    pub const STATUS_RESET: &str = "status:reset";
}

// ============================================================
// Reply
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub duration: f64,
    pub is_final: bool,

    pub character: Option<String>,
    pub role_id: Option<i32>,
    pub emotion: String,
    pub original_tag: String,
    pub message: String,
    pub tts_text: Option<String>,
    pub motion_text: Option<String>,
    pub audio_file: Option<String>,
    pub original_message: String,
    pub display_name: Option<String>,
    pub display_subtitle: Option<String>,
    /// 触发此回复的用户消息序号（1-indexed，由 sender_role_id == Some(0) 计数得出）。
    /// `None` 表示主动对话等非用户触发的回复。
    pub user_message_seq: Option<u32>,
}

impl ReplyResponse {
    pub fn new_reply() -> Self {
        Self {
            type_: "reply".to_string(),
            duration: -1.0,
            is_final: false,
            character: None,
            role_id: None,
            emotion: String::new(),
            original_tag: String::new(),
            message: String::new(),
            tts_text: None,
            motion_text: None,
            audio_file: None,
            original_message: String::new(),
            display_name: None,
            display_subtitle: None,
            user_message_seq: None,
        }
    }
}

// ============================================================
// Thinking / Error / Reset
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub is_thinking: bool,
    pub duration: f64,
}

impl ThinkingResponse {
    pub fn new(is_thinking: bool) -> Self {
        Self {
            type_: "thinking".to_string(),
            is_thinking,
            duration: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(rename = "error_code")]
    pub error_code: String,
    pub detail: String,
}

impl ErrorResponse {
    pub fn new(error_code: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            type_: "error".to_string(),
            error_code: error_code.into(),
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResetResponse {
    #[serde(rename = "type")]
    pub type_: String,
    pub status: String,
}

impl StatusResetResponse {
    pub fn new(status: impl Into<String>) -> Self {
        Self {
            type_: "status_reset".to_string(),
            status: status.into(),
        }
    }
}
