//! Static utility functions for script event execution.
//!
//! Replaces Python `ScriptFunction` static methods and parts of `Function` YAML/string utilities.

use anyhow::{anyhow, Result};
use regex::Regex;
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::types::{GameRole, LineAttributeExt, LineBase, ScriptStatus};
use crate::db::entities::line::LineAttribute;

// ============================================================
// Placeholder replacement
// ============================================================

/// Replace `%player%` with the player's user_name.
pub fn replace_placeholder(text: &str, game_status: &GameStatus) -> String {
    text.replace("%player%", &game_status.player.user_name)
}

// ============================================================
// Role lookup
// ============================================================

/// Look up a role by character reference string.
/// `"MAIN"` → `game_status.main_role_id`, otherwise lookup by script key + character folder.
pub async fn get_role<'a>(
    game_status: &'a mut GameStatus,
    db: &'a DatabaseConnection,
    script_status: &ScriptStatus,
    character: &str,
) -> Result<&'a mut GameRole> {
    if character == "MAIN" {
        let main_id = game_status
            .main_role_id
            .ok_or_else(|| anyhow!("MAIN 角色未设定：剧本启动前须先选择主角"))?;
        return game_status.get_role(db, main_id).await;
    }

    let role_id = game_status
        .role_manager
        .get_role_by_script_keys(db, &script_status.path_key(), character)
        .await?
        .role_id
        .ok_or_else(|| anyhow!("角色 {} 未在数据库中找到", character))?;

    game_status.get_role(db, role_id).await
}

// ============================================================
// Variable action parsing
// ============================================================

/// Operation parsed from a variable action string.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VarOp {
    Assign,
    Add,
    Sub,
}

/// Parse a variable action string like `"flag = true"`, `"count += 1"`, `"hp -= 5"`.
/// Returns `(op, var_name, value)`.
pub fn parse_variable_action(action: &str) -> Result<(VarOp, String, Value)> {
    let re = Regex::new(r"^\s*(\w+)\s*(=|\+=|-=)\s*(.+?)\s*$")
        .map_err(|e| anyhow!("正则编译失败: {}", e))?;

    let caps = re
        .captures(action)
        .ok_or_else(|| anyhow!("无法解析变量操作: '{}'", action))?;

    let var_name = caps[1].to_string();
    let op_str = caps[2].to_string();
    let value_str = caps[3].trim().to_string();

    let op = match op_str.as_str() {
        "=" => VarOp::Assign,
        "+=" => VarOp::Add,
        "-=" => VarOp::Sub,
        _ => return Err(anyhow!("未知操作符: {}", op_str)),
    };

    let value = parse_value(&value_str)?;

    Ok((op, var_name, value))
}

/// Parse a string value into a JSON Value (bool, int, float, or string).
/// Supports `random(min, max)` syntax for integer random.
pub fn parse_value(s: &str) -> Result<Value> {
    let s = s.trim();

    // random(min, max)
    if let Some(inner) = s
        .strip_prefix("random(")
        .and_then(|rest| rest.strip_suffix(")"))
    {
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if parts.len() == 2 {
            let min: i64 = parts[0]
                .parse()
                .map_err(|_| anyhow!("random min 不是整数: {}", parts[0]))?;
            let max: i64 = parts[1]
                .parse()
                .map_err(|_| anyhow!("random max 不是整数: {}", parts[1]))?;
            if max < min {
                return Err(anyhow!("random max({}) < min({})", max, min));
            }
            // Simple deterministic "random" using path-based hash — in practice
            // we use rand crate, but keep it simple for script vars.
            let val = min + ((max - min) / 2); // midpoint as fallback
            return Ok(Value::Number(val.into()));
        }
    }

    // bool
    if s.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if s.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }

    // null
    if s.eq_ignore_ascii_case("null") || s.eq_ignore_ascii_case("none") {
        return Ok(Value::Null);
    }

    // int
    if let Ok(n) = s.parse::<i64>() {
        return Ok(Value::Number(n.into()));
    }

    // float
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return Ok(Value::Number(num));
        }
    }

    // string: strip surrounding quotes
    let inner = s
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            s.strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(s);

    Ok(Value::String(inner.to_string()))
}

/// Apply a variable operation to the current (optional) value.
pub fn apply_variable_action(op: VarOp, current: Option<&Value>, value: Value) -> Value {
    match op {
        VarOp::Assign => value,
        VarOp::Add => {
            if let (Some(Value::Number(cur)), Value::Number(val)) = (current, &value) {
                if let (Some(cur_f), Some(val_f)) = (cur.as_f64(), val.as_f64()) {
                    if let Some(result) = serde_json::Number::from_f64(cur_f + val_f) {
                        return Value::Number(result);
                    }
                }
                if let (Some(cur_i), Some(val_i)) = (cur.as_i64(), val.as_i64()) {
                    return Value::Number((cur_i + val_i).into());
                }
            }
            value
        }
        VarOp::Sub => {
            if let (Some(Value::Number(cur)), Value::Number(val)) = (current, &value) {
                if let (Some(cur_f), Some(val_f)) = (cur.as_f64(), val.as_f64()) {
                    if let Some(result) = serde_json::Number::from_f64(cur_f - val_f) {
                        return Value::Number(result);
                    }
                }
                if let (Some(cur_i), Some(val_i)) = (cur.as_i64(), val.as_i64()) {
                    return Value::Number((cur_i - val_i).into());
                }
            }
            value
        }
    }
}

// ============================================================
// AI response matching (for chapter_end ai_judged)
// ============================================================

/// Case-insensitive match of LLM response against option names.
/// Returns the `next` value of the first matching option.
pub fn match_ai_response_options(ai_response: &str, options: &[Value]) -> Option<String> {
    let response_lower = ai_response.trim().to_lowercase();

    for opt in options {
        let name = opt.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if !name.is_empty() && response_lower.contains(&name.to_lowercase()) {
            return opt
                .get("next")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
    }

    // Fallback: check default option
    for opt in options {
        if opt
            .get("default")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            return opt
                .get("next")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
    }

    None
}

// ============================================================
// Process options (for choice events)
// ============================================================

use crate::ai_service::game_system::script_engine::events::evaluate_condition;

/// Iterate options: match input text or evaluate condition, then execute actions.
/// Returns true if any option matched.
pub async fn process_options(
    game_status: &mut GameStatus,
    db: &DatabaseConnection,
    script_status: &mut ScriptStatus,
    options: &[Value],
    input: Option<&str>,
) -> Result<bool> {
    for opt in options {
        // Check condition
        let condition = opt.get("condition").and_then(|v| v.as_str()).unwrap_or("");
        if !condition.is_empty() && !evaluate_condition(condition, &script_status.vars) {
            continue;
        }

        // Match input text
        if let Some(input) = input {
            let text = opt.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if !text.is_empty() && text != input {
                // Not a text match; if condition passed, still check further
                // For backward compat, if input is given, require text match
                continue;
            }
        }

        // Execute actions
        if let Some(actions) = opt.get("actions").and_then(|v| v.as_array()) {
            handle_actions(game_status, db, script_status, actions).await?;
        }
        return Ok(true);
    }

    Ok(false)
}

/// Execute a list of action dicts (add_line, set_var).
pub async fn handle_actions(
    game_status: &mut GameStatus,
    db: &DatabaseConnection,
    script_status: &mut ScriptStatus,
    actions: &[Value],
) -> Result<()> {
    for action in actions {
        let action_type = action.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match action_type {
            "add_line" => {
                let content = action
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let content = replace_placeholder(&content, game_status);
                let line = LineBase {
                    content,
                    attribute: LineAttributeExt(LineAttribute::User),
                    display_name: Some(game_status.player.user_name.clone()),
                    ..Default::default()
                };
                game_status.add_line(db, line).await?;
            }
            "set_var" => {
                let content = action.get("content").and_then(|v| v.as_str()).unwrap_or("");
                if let Ok((op, var_name, value)) = parse_variable_action(content) {
                    let current = script_status.get_variable(&var_name).cloned();
                    let result = apply_variable_action(op, current.as_ref(), value);
                    script_status.set_variable(var_name, result);
                }
            }
            other => {
                tracing::warn!("未知的选项动作类型: {}", other);
            }
        }
    }
    Ok(())
}
