//! 上帝 Agent 的工具（function）定义与解析。
//!
//! 目前仅包含 `select_next_speaker` 工具。后续扩展更多工具时在此注册。

use crate::ai_service::types::{parse_tool_args, ToolCall, ToolDefinition};

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
///
/// 对 `arguments` 做容错归一化（嵌套 `{"arguments": {...}}` / 双编码 JSON / 非法 JSON），
/// 并对 `role_id` 做类型容忍：部分模型会把整数输出成字符串（`"6"`）或浮点（`6.0`），
/// 统一解析为 `i32`。返回 `None` 表示无法解析（role_id 缺失或无法转成整数）。
pub fn parse_speaker_selection(tool_call: &ToolCall) -> Option<(i32, String)> {
    let args = parse_tool_args(&tool_call.function.arguments);

    let role_id = args.get("role_id").and_then(parse_role_id)? as i32;
    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("（无理由）")
        .to_string();

    Some((role_id, reason))
}

/// 容错解析 `role_id`：接受整数（`6`）、浮点（`6.0`）、字符串（`"6"` / `" 6 "` / `"6.0"`）。
fn parse_role_id(v: &serde_json::Value) -> Option<i64> {
    if let Some(i) = v.as_i64() {
        return Some(i);
    }
    if let Some(f) = v.as_f64() {
        return Some(f as i64);
    }
    let s = v.as_str()?.trim();
    s.parse::<i64>()
        .ok()
        .or_else(|| s.parse::<f64>().ok().map(|f| f as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc(arguments: &str) -> ToolCall {
        ToolCall {
            id: "call-1".into(),
            type_: "function".into(),
            function: crate::ai_service::types::FunctionCall {
                name: "select_next_speaker".into(),
                arguments: arguments.into(),
            },
        }
    }

    #[test]
    fn parses_integer_role_id() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":6,"reason":"x"}"#));
        assert_eq!(r, Some((6, "x".to_string())));
    }

    #[test]
    fn parses_string_role_id() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":"6","reason":"x"}"#));
        assert_eq!(r, Some((6, "x".to_string())));
    }

    #[test]
    fn parses_string_role_id_with_whitespace() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":" 6 ","reason":"x"}"#));
        assert_eq!(r, Some((6, "x".to_string())));
    }

    #[test]
    fn parses_float_role_id() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":6.0,"reason":"x"}"#));
        assert_eq!(r, Some((6, "x".to_string())));
    }

    #[test]
    fn parses_string_float_role_id() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":"6.0","reason":"x"}"#));
        assert_eq!(r, Some((6, "x".to_string())));
    }

    #[test]
    fn parses_nested_arguments_shape() {
        let r = parse_speaker_selection(&tc(r#"{"arguments":{"role_id":6,"reason":"x"}}"#));
        assert_eq!(r, Some((6, "x".to_string())));
    }

    #[test]
    fn parses_player_zero() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":0,"reason":"还给玩家"}"#));
        assert_eq!(r, Some((0, "还给玩家".to_string())));
    }

    #[test]
    fn missing_reason_defaults() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":6}"#));
        assert_eq!(r, Some((6, "（无理由）".to_string())));
    }

    #[test]
    fn rejects_non_numeric_role_id() {
        let r = parse_speaker_selection(&tc(r#"{"role_id":"abc","reason":"x"}"#));
        assert_eq!(r, None);
    }

    #[test]
    fn rejects_missing_role_id() {
        let r = parse_speaker_selection(&tc(r#"{"reason":"x"}"#));
        assert_eq!(r, None);
    }

    #[test]
    fn rejects_non_object_arguments() {
        let r = parse_speaker_selection(&tc("not valid json"));
        assert_eq!(r, None);
    }
}
