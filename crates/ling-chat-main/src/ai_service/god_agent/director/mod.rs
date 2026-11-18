//! 导演 Agent：上帝 Agent 的交互会话分支。
//!
//! 玩家在主聊天页右侧侧栏里直接和导演沟通剧情走向。它与上帝 Agent 共用同一个
//! LLM 槽位，会话绑定当前存档（自由对话只在内存），每轮现装配一次现场上下文。
//! 步骤二起接入确定性剧本事件工具（见 event_tools），自由对话下由运行层闸门放行。

pub mod core;
pub mod event_tools;
pub mod tools;

use serde::Serialize;

use crate::ai_service::types::ToolCall;

pub use core::{AssembledSystem, DirectorAgent, DirectorSession, TurnContext};

/// 一次导演对话请求的事件流，经 `tauri::ipc::Channel` 推送前端。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DirectorAgentEvent {
    /// 运行状态（如「已停止生成」）。
    Status { content: String },
    /// 正文增量。
    MessageDelta { content: String },
    /// 思考链增量。
    Reasoning { content: String },
    /// 一个工具即将被调用。
    ToolCall {
        call_id: String,
        tool: String,
        args: serde_json::Value,
    },
    /// 工具执行结果。
    ToolResult {
        call_id: String,
        tool: String,
        ok: bool,
        output: String,
        error: Option<String>,
    },
    /// 本轮结束。`message_id` 为 0 表示没有落库（自由对话或空回复）。
    Done { final_text: String, message_id: i32 },
    /// 致命错误。
    Error { message: String },
}

/// 一条导演消息。`id` 为 0 表示尚未落库。
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

/// 侧栏打开 / 切档后拉取的整体状态。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorState {
    /// 是否有可用 LLM；false 时前端只显示空态。
    pub available: bool,
    /// 是否绑定存档（false 表示消息只在内存）。
    pub save_bound: bool,
    pub save_id: Option<i32>,
    pub running: bool,
    pub messages: Vec<DirectorMessage>,
}

/// system 的分段视图，供「查看记忆」分区展示。
#[derive(Debug, Clone, Serialize)]
pub struct ContextSection {
    pub key: String,
    pub title: String,
    pub content: String,
}

/// 一次请求的消息视图（role + 正文 + 工具字段）。
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

/// 装配后的上下文统计。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextStats {
    pub system_chars: usize,
    pub history_chars: usize,
    pub message_count: usize,
    /// 因历史预算被整块裁掉的消息数。
    pub dropped_messages: usize,
    /// 是否发生过截断（台词段或历史预算）。
    pub truncated: bool,
}

/// 一次请求的上下文快照。上次实际请求的那份只存内存，重启即失。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorContext {
    /// last_run（上次实际请求）或 rebuilt（按当前现场重新装配）
    pub source: String,
    pub built_at: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub system_prompt: String,
    pub sections: Vec<ContextSection>,
    pub messages: Vec<ContextMessage>,
    pub stats: ContextStats,
}
