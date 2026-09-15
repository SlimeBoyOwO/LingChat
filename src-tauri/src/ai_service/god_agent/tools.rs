//! 上帝 Agent 的工具（function）定义与解析。
//!
//! 目前包含 `select_next_speaker`（发言者选择）与 `update_affection`
//! （好感度定期评估）。后续扩展更多工具时在此注册。

use crate::ai_service::types::{AffectionVector, NegativeVector, ToolCall, ToolDefinition, parse_tool_args};

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

// ============================================================
// update_affection（好感度定期评估）
// ============================================================

/// 获取「调整角色对玩家的情感维度」工具定义。
pub fn update_affection_tool() -> ToolDefinition {
    ToolDefinition::new(
        "update_affection",
        "根据最近的对话，调整指定角色对玩家的情感维度。只对确实受到这段对话影响的维度给出增量：\
         日常正面互动 +1~+2，明显打动/冒犯 ±3，非常深刻或严重伤害 ±4~+5/-4~-5；\
         没有明显变化的维度不要包含。负面情绪用 negative_deltas：被冒犯/忽视/伤害时增加对应维度，\
         被安抚/取悦时减少对应维度（下限 0）。每个有情感变化的在场角色各调用一次本工具，\
         完全没有变化的角色不要调用。好感度数值不设上限，可以超过 100（满溢），也可以为负数（疏离）。",
        serde_json::json!({
            "type": "object",
            "properties": {
                "role_id": {
                    "type": "integer",
                    "description": "要调整的角色 role_id（不可为 0，0 是玩家）。"
                },
                "deltas": {
                    "type": "object",
                    "description": "好感各维度增量（-5~+5 的整数），只包含需要变化的维度。",
                    "properties": {
                        "fondness": {"type": "integer", "description": "好感增量"},
                        "trust": {"type": "integer", "description": "信赖增量"},
                        "intimacy": {"type": "integer", "description": "亲密增量"},
                        "rapport": {"type": "integer", "description": "默契增量"},
                        "interest": {"type": "integer", "description": "兴趣增量"},
                        "longing": {"type": "integer", "description": "思念增量"}
                    }
                },
                "negative_deltas": {
                    "type": "object",
                    "description": "负面情绪各维度增量（-5~+5 的整数），被冒犯/伤害时为正、被安抚时为负，只包含需要变化的维度。",
                    "properties": {
                        "anger": {"type": "integer", "description": "愤怒增量"},
                        "hurt": {"type": "integer", "description": "受伤增量"},
                        "disappointment": {"type": "integer", "description": "失望增量"},
                        "indifference": {"type": "integer", "description": "冷漠增量"},
                        "jealousy": {"type": "integer", "description": "嫉妒增量"},
                        "estrangement": {"type": "integer", "description": "疏远增量"}
                    }
                },
                "reason": {
                    "type": "string",
                    "description": "调整的简短理由（中文）。"
                }
            },
            "required": ["role_id", "reason"]
        }),
    )
}

/// 一次好感度调整：role_id + 好感维度增量 + 负面维度增量 + 理由。
#[derive(Clone, Debug)]
pub struct AffectionAdjustment {
    pub role_id: i32,
    /// 好感（维度键名, 增量），增量已钳制到 ±5、剔除了 0 与未知维度。
    pub deltas: Vec<(String, i32)>,
    /// 负面情绪（维度键名, 增量），同样的钳制规则。
    pub negative_deltas: Vec<(String, i32)>,
    pub reason: String,
}

/// 解析一个增量对象（容错双编码 JSON 字符串），按合法维度表过滤并钳制 ±5。
fn parse_deltas_object(
    raw: Option<&serde_json::Value>,
    valid: &[(&'static str, &'static str)],
) -> Vec<(String, i32)> {
    let Some(raw) = raw else { return Vec::new() };
    let value = if raw.is_object() {
        raw.clone()
    } else {
        parse_tool_args(&raw.to_string())
    };
    let mut deltas = Vec::new();
    if let Some(map) = value.as_object() {
        for (dim, raw) in map {
            let Some(d) = parse_delta(raw) else { continue };
            let d = d.clamp(-5, 5);
            if d != 0 && valid.iter().any(|(k, _)| k == dim) {
                deltas.push((dim.clone(), d));
            }
        }
    }
    deltas
}

/// 从 tool call 解析好感度调整。role_id 为 0、deltas 与 negative_deltas 都为空时返回 None。
pub fn parse_affection_update(tool_call: &ToolCall) -> Option<AffectionAdjustment> {
    let args = parse_tool_args(&tool_call.function.arguments);

    let role_id = args.get("role_id").and_then(parse_role_id)? as i32;
    if role_id == 0 {
        return None;
    }

    let deltas = parse_deltas_object(args.get("deltas"), &AffectionVector::DIMENSIONS);
    let negative_deltas =
        parse_deltas_object(args.get("negative_deltas"), &NegativeVector::DIMENSIONS);

    if deltas.is_empty() && negative_deltas.is_empty() {
        return None;
    }

    let reason = args
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("（无理由）")
        .to_string();

    Some(AffectionAdjustment {
        role_id,
        deltas,
        negative_deltas,
        reason,
    })
}

/// 容错解析维度增量：接受整数、浮点、字符串。
fn parse_delta(v: &serde_json::Value) -> Option<i32> {
    if let Some(i) = v.as_i64() {
        return Some(i as i32);
    }
    if let Some(f) = v.as_f64() {
        return Some(f as i32);
    }
    let s = v.as_str()?.trim();
    s.parse::<i32>()
        .ok()
        .or_else(|| s.parse::<f64>().ok().map(|f| f as i32))
}
