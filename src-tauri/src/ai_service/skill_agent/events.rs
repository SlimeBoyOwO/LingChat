//! Skill Agent 流式事件，经 `tauri::ipc::Channel<SkillAgentEvent>` 推送前端。

use serde::Serialize;

/// Skill Agent 运行期间推送到前端的流式事件。
///
/// 与 ling_chat_agent 的 `ChatEvent` 同构，但增加 `Reasoning`（LingChat 的
/// LlmClient 会产出思考链片段）。事件走作用域隔离的 `Channel`，不经全局
/// `emit`，因此与剧本编辑器试玩的 `preview_generation` 守卫互不干扰。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillAgentEvent {
    /// 运行生命周期状态（如「思考中…」「已停止生成」）。
    Status {
        content: String,
    },
    /// 流式文本增量。
    MessageDelta {
        content: String,
    },
    /// 思考链增量（仅统计展示，不进入正式回复）。
    Reasoning {
        content: String,
    },
    /// 一个工具即将被调用。
    ToolCall {
        call_id: String,
        tool: String,
        /// 归一化后的参数对象。
        args: serde_json::Value,
        /// LLM 返回的原始参数 JSON 字符串，可能被截断或非法。
        raw_args: String,
    },
    /// 工具执行结果。
    ToolResult {
        call_id: String,
        tool: String,
        ok: bool,
        output: String,
        error: Option<String>,
    },
    /// 命令需要用户审批。
    PendingApproval {
        request_id: String,
        tool: String,
        args: serde_json::Value,
    },
    /// 本轮对话结束。
    Done {
        final_text: String,
        /// 本轮累计 token 用量；provider 未上报时为 `None`。
        usage: Option<Usage>,
    },
    /// 致命错误。
    Error {
        message: String,
    },
}

/// Token 用量。
#[derive(Debug, Clone, Default, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}
