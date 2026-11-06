//! 上帝 Agent 的两个决策工具：`select_next_speaker`（选下一个发言者）与
//! `update_affection`（好感度定期评估的落地）。
//!
//! 它们不进聊天的 `built_in_registry`，而是由 [`god_agent_registry`] 单独注册，
//! 避免出现在工具权限页与 `tool_permissions.toml` 里——这两个能力由上帝 Agent
//! 自己的 LLM 驱动，不由用户侧聊天 LLM 选择，没有权限矩阵语义。
//!
//! 两者都用宽松参数模式：模型常常把整数写成 "6"/6.0，或把参数包成
//! `{"arguments": {...}}` / 双编码 JSON 字符串，工具内自行容错解析。

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{Value, json};
use tauri::{Emitter, Manager};

use crate::AppState;
use crate::ai_service::affection::{self, AffectionChangedPayload, AffectionState};
use crate::ai_service::types::{AffectionVector, NegativeVector, ToolDefinition};

use crate::ai_service::tools::executor::{Tool, ToolContext, ToolError, ToolResult};
use crate::ai_service::tools::game_status_handle;
use crate::ai_service::tools::registry::ToolRegistry;

pub const SELECT_NEXT_SPEAKER: &str = "select_next_speaker";
pub const UPDATE_AFFECTION: &str = "update_affection";
pub const RECOMMEND_TOOLS: &str = "recommend_tools";

/// 构建上帝 Agent 的专用注册表。
pub fn god_agent_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();
    registry
        .register(Arc::new(SelectNextSpeaker))
        .expect("注册 select_next_speaker 失败");
    registry
        .register(Arc::new(UpdateAffection))
        .expect("注册 update_affection 失败");
    registry
        .register(Arc::new(RecommendTools))
        .expect("注册 recommend_tools 失败");
    registry
}

// ============================================================
// select_next_speaker
// ============================================================

/// 选择下一个发言角色：直接把发言权交给该角色（只改说话者，不做角色加载与在场集重建）。
pub struct SelectNextSpeaker;

#[async_trait]
impl Tool for SelectNextSpeaker {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            SELECT_NEXT_SPEAKER,
            "在多人对话中，根据当前的对话上下文、角色性格和对话流向，选择最适合接下来发言的角色。\
             如果对话已经自然结束、或应该由玩家来发言了，请选择 role_id=0 来把发言权交还给玩家。\
             如果某个非玩家角色说完了话、话题还没有结束并且另一个非玩家角色有很强的接话动机，则选择该非玩家角色的 role_id。",
            json!({
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

    fn lenient_arguments(&self) -> bool {
        true
    }

    /// 执行内要走 `get_role`（DB），执行器默认的 2 秒太紧。
    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(15))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let role_id = arguments
            .get("role_id")
            .and_then(parse_role_id)
            .map(|id| id as i32)
            .ok_or_else(|| {
                ToolError::InvalidArguments("select_next_speaker 需要整数 role_id".into())
            })?;
        let reason = arguments
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("（无理由）")
            .to_string();

        // role_id=0 表示把发言权交还玩家，保持现状、不改任何状态。
        if role_id == 0 {
            return Ok(json!({"ok": true, "role_id": 0, "reason": reason}));
        }

        let app = context.require_app()?;
        let state = app.state::<AppState>();
        let game_status = game_status_handle(&app).await;

        let name = {
            let mut gs = game_status.lock().await;
            if !gs.present_role_ids.contains(&role_id) {
                return Err(ToolError::Execution(format!(
                    "上帝Agent 选择了不在场的角色 {role_id}，在场角色: {:?}",
                    gs.present_role_ids
                )));
            }
            gs.current_role_id = Some(role_id);
            gs.get_role(&state.db, role_id)
                .await
                .map_err(|e| ToolError::Execution(format!("加载角色 {role_id} 失败: {e}")))?
                .display_name
                .clone()
                .unwrap_or_else(|| format!("角色{role_id}"))
        };

        // 与前端 character:switch 的契约保持一致（与 character_switch 工具同一事件）。
        let payload = json!({
            "type": "character_switch",
            "roleId": role_id,
            "characterName": name,
        });
        if let Err(e) = app.emit(
            crate::ai_service::game_system::world_actions::EVENT_CHARACTER_SWITCH,
            &payload,
        ) {
            tracing::warn!("emit character:switch 失败: {e}");
        }

        Ok(json!({"ok": true, "role_id": role_id, "name": name, "reason": reason}))
    }
}

// ============================================================
// update_affection
// ============================================================

/// 调整指定角色对玩家的情感维度，并就地落库、广播、写旁白台词。
pub struct UpdateAffection;

#[async_trait]
impl Tool for UpdateAffection {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            UPDATE_AFFECTION,
            "根据最近的对话，调整指定角色对玩家的情感维度。只对确实受到这段对话影响的维度给出增量：\
             日常正面互动 +1~+2，明显打动/冒犯 ±3，非常深刻或严重伤害 ±4~+5/-4~-5；\
             没有明显变化的维度不要包含。负面情绪用 negative_deltas：被冒犯/忽视/伤害时增加对应维度，\
             被安抚/取悦时减少对应维度（下限 0）。每个有情感变化的在场角色各调用一次本工具，\
             完全没有变化的角色不要调用。好感度数值不设上限，可以超过 100（满溢），也可以为负数（疏离）。",
            json!({
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

    fn lenient_arguments(&self) -> bool {
        true
    }

    /// 执行内既有内存改值，也有 `set_variable` 落档与 `add_line` 写库；
    /// 执行器默认的 2 秒会把写库截断，留下「六维已改、档案已写、旁白丢失」的半完成态。
    fn timeout_hint(&self) -> Option<Duration> {
        Some(Duration::from_secs(30))
    }

    async fn execute(
        &self,
        context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        let Some(role_id) = arguments
            .get("role_id")
            .and_then(parse_role_id)
            .map(|id| id as i32)
        else {
            return Err(ToolError::InvalidArguments(
                "update_affection 需要整数 role_id".into(),
            ));
        };
        let reason = arguments
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or("（无理由）")
            .to_string();

        let deltas = parse_deltas_object(arguments.get("deltas"), &AffectionVector::DIMENSIONS);
        let negative_deltas = parse_deltas_object(
            arguments.get("negative_deltas"),
            &NegativeVector::DIMENSIONS,
        );

        // role_id=0（玩家）或没有任何有效增量：不是错误，直接跳过。
        if role_id == 0 || (deltas.is_empty() && negative_deltas.is_empty()) {
            return Ok(json!({"ok": false, "skipped": true, "role_id": role_id}));
        }

        let app = context.require_app()?;
        let game_status = game_status_handle(&app).await;
        let mut gs = game_status.lock().await;

        if !gs.present_role_ids.contains(&role_id) {
            return Ok(json!({"ok": false, "skipped": true, "role_id": role_id}));
        }

        let Some((values, negative)) =
            gs.role_manager
                .adjust_affection(role_id, &deltas, &negative_deltas)
        else {
            // 角色未加载：与内存调整的既有语义一致，静默跳过。
            return Ok(json!({"ok": false, "skipped": true, "role_id": role_id}));
        };

        // 持久化到本存档的全局变量 JSON（跟随存档保存）。这是好感度唯一的落盘点。
        let identity_id = gs.player.identity_id.clone();
        gs.set_variable(
            affection::var_key_for(role_id, identity_id.as_deref()),
            affection::state_to_value(&AffectionState {
                total: values.average(),
                vector: values,
                negative,
            }),
        );

        let payload = AffectionChangedPayload {
            role_id,
            deltas: deltas.iter().cloned().collect(),
            negative_deltas: negative_deltas.iter().cloned().collect(),
            average: values.average(),
            values,
            negative,
            reason: reason.clone(),
        };
        tracing::info!(
            "[Affection] role_id={} 调整 {:?}（{}）→ 平均 {}",
            payload.role_id,
            payload.deltas,
            payload.reason,
            payload.average,
        );
        let _ = app.emit("affection:changed", payload);

        // 旁白文本交给评估能力层排入近期记忆，后台任务不直接改台词表。
        let name = gs
            .role_manager
            .get_loaded(role_id)
            .and_then(|r| r.display_name.clone())
            .unwrap_or_else(|| format!("角色{role_id}"));
        let player_name = gs.player.user_name.clone();
        let line_text =
            affection::describe_change_for_line(&name, &player_name, &values, &negative);

        Ok(json!({
            "ok": true,
            "role_id": role_id,
            "average": values.average(),
            "reason": reason,
            "line_text": line_text,
        }))
    }
}

// ============================================================
// recommend_tools
// ============================================================

/// 本轮可用工具的推荐结果：只把挑中的名字回显给调用方，不改任何状态。
///
/// 与另外两个决策工具不同，它不碰 `AppState`——推荐是「一句话提示」，
/// 真正让工具可调用的是调用方把这几个名字并进本轮工具集。
pub struct RecommendTools;

#[async_trait]
impl Tool for RecommendTools {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            RECOMMEND_TOOLS,
            "从给定的工具清单里挑出与这次对话相关的那几件，供对话模型参考。\
             只挑能直接帮上忙的；都不相关就把 names 留空。不要挑工具清单以外的名字。",
            json!({
                "type": "object",
                "properties": {
                    "names": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "推荐的工具名，必须来自清单，最多 5 个；都不相关就传空数组。"
                    },
                    "reason": {
                        "type": "string",
                        "description": "为什么推荐它们（一句中文）。"
                    }
                },
                "required": ["names", "reason"]
            }),
        )
    }

    fn lenient_arguments(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        _context: &ToolContext,
        arguments: Value,
    ) -> Result<ToolResult, ToolError> {
        Ok(json!({
            "ok": true,
            "names": parse_names(arguments.get("names")),
            "reason": arguments
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("（无理由）"),
        }))
    }
}

/// 容错解析工具名列表：数组取字符串元素，单个字符串按顿号/逗号拆开。
fn parse_names(raw: Option<&Value>) -> Vec<String> {
    let mut names = Vec::new();
    match raw {
        Some(Value::Array(items)) => {
            names.extend(
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|name| name.trim().to_string()),
            );
        },
        Some(Value::String(text)) => {
            names.extend(
                text.split(['、', ',', '，', ' ', '\n'])
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_string),
            );
        },
        _ => {},
    }
    names.retain(|name| !name.is_empty());
    names
}

// ============================================================
// 容错解析
// ============================================================

/// 容错解析 `role_id`：接受整数（`6`）、浮点（`6.0`）、字符串（`"6"` / `" 6 "` / `"6.0"`）。
pub(crate) fn parse_role_id(v: &Value) -> Option<i64> {
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

/// 容错解析维度增量：接受整数、浮点、字符串。
fn parse_delta(v: &Value) -> Option<i32> {
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

/// 解析一个增量对象（容错双编码 JSON 字符串），按合法维度表过滤并钳制 ±5。
fn parse_deltas_object(
    raw: Option<&Value>,
    valid: &[(&'static str, &'static str)],
) -> Vec<(String, i32)> {
    let Some(raw) = raw else { return Vec::new() };
    let value = if raw.is_object() {
        raw.clone()
    } else {
        crate::ai_service::types::parse_tool_args(&raw.to_string())
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
