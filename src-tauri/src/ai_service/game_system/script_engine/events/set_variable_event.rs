//! Set variable event — parses and applies variable assignments to ScriptStatus.
//!
//! # Python Parity
//! Python's `SetVariableEvent` overrode `execute()` instead of `_execute()`,
//! making it silently non-functional. This Rust version correctly implements
//! the `ScriptEvent::execute()` trait method.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::utils::script_function::{
    apply_variable_action, parse_variable_action,
};

pub struct SetVariableEvent {
    options: Vec<Value>,
    duration: Option<f64>,
}

impl SetVariableEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            options: data
                .get("options")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for SetVariableEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let mut gs = ctx.game_status.lock().await;
        let script_status = gs
            .script_status
            .as_mut()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法设置变量"))?;

        for opt in &self.options {
            // Check condition
            let condition = opt.get("condition").and_then(|v| v.as_str()).unwrap_or("");
            if !condition.is_empty() {
                if !crate::ai_service::game_system::script_engine::events::evaluate_condition(
                    condition,
                    &script_status.vars,
                ) {
                    continue;
                }
            }

            // Process actions
            if let Some(actions) = opt.get("actions").and_then(|v| v.as_array()) {
                for action in actions {
                    let action_type = action.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    if action_type == "set_var" {
                        let content = action.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        if let Ok((op, var_name, value)) = parse_variable_action(content) {
                            let current = script_status.get_variable(&var_name).cloned();
                            let result = apply_variable_action(op, current.as_ref(), value);
                            script_status.set_variable(var_name, result);
                            tracing::info!(
                                "[SetVariableEvent] {} = {:?}",
                                content,
                                script_status.get_variable(
                                    &content
                                        .split(&['=', '+', '-'][..])
                                        .next()
                                        .unwrap_or("")
                                        .trim()
                                )
                            );
                        } else {
                            tracing::warn!("[SetVariableEvent] 无法解析操作: '{}'", content);
                        }
                    } else {
                        // 这里只实现 set_var。`choices` 的 handle_actions 还支持
                        // add_line，作者很容易以为两处通用 —— 原先写错会被静默丢弃。
                        tracing::warn!(
                            "[SetVariableEvent] 忽略不支持的动作类型 '{}'（此事件仅支持 set_var）",
                            action_type
                        );
                    }
                }
            }
        }

        Ok(None)
    }

    fn event_type() -> &'static str {
        "set_variable"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(SetVariableEvent::event_type(), |data| {
        Box::new(SetVariableEvent::from_event_data(&data))
    });
}
