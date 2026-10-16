//! Skill Agent 流式事件，经 `tauri::ipc::Channel<SkillAgentEvent>` 推送前端。

use serde::Serialize;

/// Skill Agent 运行期间推送到前端的流式事件。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SkillAgentEvent {
    /// 运行生命周期状态（如「思考中…」「已停止生成」）。
    Status { content: String },
    /// 流式文本增量。
    MessageDelta { content: String },
    /// 一项的开始（一轮可能逐项做多件事）：带的是这一项的标题文本（前端当正文用），语义是分段边界。
    ItemStart {
        /// 队列里的位置（从 1 起）与本轮共几项；给将来的界面用，前端现在只渲染 `title`。
        index: usize,
        total: usize,
        title: String,
    },
    /// 思考链增量（仅统计展示，不进入正式回复）。
    Reasoning { content: String },
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
    #[cfg_attr(not(desktop), allow(dead_code))]
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
    /// 会话标题已自动生成（首轮回复结束后由后台任务生成，经此事件通知前端刷新列表）。
    ConversationTitle { title: String },
    /// 致命错误。
    Error { message: String },
}

/// Token 用量。
#[derive(Debug, Clone, Default, Serialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    /// 输入中命中缓存（cache read）的 token 数；provider 未上报缓存时为 0。
    pub cached_tokens: u64,
}
