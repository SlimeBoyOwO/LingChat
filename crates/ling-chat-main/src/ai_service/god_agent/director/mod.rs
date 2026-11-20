//! 导演会话与共享视图类型。

pub mod context;
pub mod core;
pub mod draft;
pub mod event_tools;
pub mod tools;

use serde::Serialize;

use crate::ai_service::types::ToolCall;

pub use context::AssembledSystem;
pub use core::{DirectorAgent, DirectorSession, TurnContext};
pub use draft::{DirectorConfirmResult, DirectorDraftView, DirectorModeView};

/// 导演对话的流式事件。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DirectorAgentEvent {
    Status {
        content: String,
    },
    MessageDelta {
        content: String,
    },
    Reasoning {
        content: String,
    },
    ToolCall {
        call_id: String,
        tool: String,
        args: serde_json::Value,
    },
    ToolResult {
        call_id: String,
        tool: String,
        ok: bool,
        output: String,
        error: Option<String>,
    },
    /// message_id 为 0 表示没有落库。
    Done {
        final_text: String,
        message_id: i32,
    },
    Error {
        message: String,
    },
}

/// id 为 0 表示尚未落库。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorMessage {
    pub id: i32,
    pub role: String,
    pub content: String,
    pub reasoning: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
    pub created_at: String,
}

impl DirectorMessage {
    pub fn to_llm(&self) -> crate::ai_service::types::LlmMessage {
        crate::ai_service::types::LlmMessage {
            role: self.role.clone(),
            content: self.content.clone(),
            tool_calls: self.tool_calls.clone(),
            tool_call_id: self.tool_call_id.clone(),
            image_data_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorState {
    pub available: bool,
    pub save_bound: bool,
    pub save_id: Option<i32>,
    pub running: bool,
    pub messages: Vec<DirectorMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSection {
    pub key: String,
    pub title: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(rename = "tool_calls")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(rename = "tool_call_id")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextStats {
    pub system_chars: usize,
    pub history_chars: usize,
    pub message_count: usize,
    pub dropped_messages: usize,
    pub truncated: bool,
}

/// 上下文快照仅保存在内存。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorContext {
    pub source: String,
    pub built_at: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub system_prompt: String,
    pub sections: Vec<ContextSection>,
    pub messages: Vec<ContextMessage>,
    pub stats: ContextStats,
}
