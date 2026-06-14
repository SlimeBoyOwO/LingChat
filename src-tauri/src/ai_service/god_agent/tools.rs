//! 上帝 Agent 的工具（function）定义与解析。
//!
//! 目前仅包含 `select_next_speaker` 工具。后续扩展更多工具时在此注册。

use crate::ai_service::types::{ToolCall, ToolDefinition};

// ============================================================
// 工具定义
// ============================================================

/// 获取 "选择下一个说话角色" 的工具定义。
pub fn select_next_speaker_tool() -> ToolDefinition {
    ToolDefinition::new(
        "select_next_speaker",
        "在多人对话中，根据当前的对话上下文、角色性格和对话流向，选择最适合接下来发言的角色。\
         如果对话已经自然结束、或应该由玩家来发言了，请选择 role_id=0 来把发言权交还给玩家。\
         如果某个非玩家角色说完了话、话题还没有结束并且另一个非玩家角色有很强的接话动机，则选择该非玩家角色的 role_id。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "role_id": {
                    "type": "integer",
                    "description": "下一个发言的角色 role_id。0 表示玩家（用户），表示将发言权交还给玩家。"
                },
                "reason": {
                    "type": "string",
                    "description": "选择该角色的简短理由（中文）。"
                }
            },
            "required": ["role_id", "reason"]
        }),
    )
}

// ============================================================
// 解析
// ============================================================

/// 从 tool call 结果中解析出选中的 role_id 和理由。
/// 返回 `None` 表示无法解析（arguments 格式异常）。
pub fn parse_speaker_selection(tool_call: &ToolCall) -> Option<(i32, String)> {
    let args: serde_json::Value =
        serde_json::from_str(&tool_call.function.arguments).ok()?;

    let role_id = args.get("role_id")?.as_i64()? as i32;
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("（无理由）")
        .to_string();

    Some((role_id, reason))
}
