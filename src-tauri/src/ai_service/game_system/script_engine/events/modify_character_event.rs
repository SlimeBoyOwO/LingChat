//! Modify character event — emotion, clothes, show/hide, perceive changes.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_MODIFY_CHARACTER, ModifyCharacterPayload,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;

pub struct ModifyCharacterEvent {
    character: String,
    emotion: Option<String>,
    action: Option<String>,
    clothes: Option<String>,
    perceive: Option<bool>,
}

impl ModifyCharacterEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            character: data
                .get("character")
                .and_then(|v| v.as_str())
                .unwrap_or("MAIN")
                .to_string(),
            emotion: data
                .get("emotion")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            action: data
                .get("action")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            clothes: data
                .get("clothes")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            perceive: data.get("perceive").and_then(loose_bool),
        }
    }
}

/// Read a YAML value that authors write as a boolean.
///
/// `perceive: true` parses as [`Value::Bool`] while `perceive: "true"` parses as
/// [`Value::String`]. The original code only handled the string form via
/// `as_str()`, so the unquoted boolean — which is what every shipped script
/// actually writes — yielded `None` and the whole perceive branch was skipped.
///
/// Returns `None` for anything unrecognised, which means "not specified" and
/// leaves the character's perception untouched. (The old code mapped every
/// non-`"true"` string to `false`, i.e. silently *removed* the character from
/// the perceiving set on a typo.)
fn loose_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => {
            let s = s.trim();
            if s.eq_ignore_ascii_case("true") {
                Some(true)
            } else if s.eq_ignore_ascii_case("false") {
                Some(false)
            } else {
                tracing::warn!("[ModifyCharacterEvent] 无法识别的 perceive 值: '{}'，已忽略", s);
                None
            }
        }
        other => {
            tracing::warn!("[ModifyCharacterEvent] perceive 应为布尔值，实际为: {}", other);
            None
        }
    }
}

#[async_trait]
impl ScriptEvent for ModifyCharacterEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let script_status = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;

        let role_id = {
            let mut gs = ctx.game_status.lock().await;
            let role = script_function::get_role(&mut *gs, ctx.db, &script_status, &self.character)
                .await?;
            let id = role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?;

            // Apply clothes (while we have mutable access to role)
            if let Some(ref clothes) = self.clothes {
                role.current_clothes = clothes.clone();
            }
            id
        };

        // Apply action: show_character / hide_character
        if let Some(ref action) = self.action {
            match action.as_str() {
                "show_character" => {
                    ctx.game_status.lock().await.onstage_role(role_id);
                }
                "hide_character" => {
                    ctx.game_status.lock().await.offstage_role(role_id);
                }
                _ => {}
            }
        }

        // Apply perceive
        if let Some(perceive) = self.perceive {
            if perceive {
                ctx.game_status
                    .lock()
                    .await
                    .present_role_ids
                    .insert(role_id);
            } else {
                ctx.game_status
                    .lock()
                    .await
                    .present_role_ids
                    .remove(&role_id);
            }
        }

        // Emit modify_character event
        let payload = ModifyCharacterPayload {
            character_id: role_id,
            emotion: self.emotion.clone(),
            action: self.action.clone(),
            clothes: self.clothes.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_MODIFY_CHARACTER, &payload);

        tracing::info!(
            "[ModifyCharacterEvent] role={} action={:?} emotion={:?}",
            role_id,
            self.action,
            self.emotion
        );
        Ok(None)
    }

    fn event_type() -> &'static str {
        "modify_character"
    }
}

pub fn register() {
    register_event(ModifyCharacterEvent::event_type(), |data| {
        Box::new(ModifyCharacterEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::{loose_bool, ModifyCharacterEvent};
    use serde_json::json;

    /// The shipped scripts all write `perceive: true` unquoted. Before PR1 this
    /// parsed to `None` and the perceive branch never ran.
    #[test]
    fn perceive_accepts_yaml_booleans() {
        let e = ModifyCharacterEvent::from_event_data(&json!({ "perceive": true }));
        assert_eq!(e.perceive, Some(true));
        let e = ModifyCharacterEvent::from_event_data(&json!({ "perceive": false }));
        assert_eq!(e.perceive, Some(false));
    }

    #[test]
    fn perceive_still_accepts_quoted_strings() {
        let e = ModifyCharacterEvent::from_event_data(&json!({ "perceive": "true" }));
        assert_eq!(e.perceive, Some(true));
        let e = ModifyCharacterEvent::from_event_data(&json!({ "perceive": " False " }));
        assert_eq!(e.perceive, Some(false));
    }

    #[test]
    fn perceive_absent_or_unparseable_is_none() {
        let e = ModifyCharacterEvent::from_event_data(&json!({}));
        assert_eq!(e.perceive, None);
        let e = ModifyCharacterEvent::from_event_data(&json!({ "perceive": "yes" }));
        assert_eq!(e.perceive, None);
        let e = ModifyCharacterEvent::from_event_data(&json!({ "perceive": 1 }));
        assert_eq!(e.perceive, None);
    }

    #[test]
    fn defaults_match_the_engine_contract() {
        let e = ModifyCharacterEvent::from_event_data(&json!({}));
        assert_eq!(e.character, "MAIN");
        assert_eq!(e.action, None);
        assert_eq!(e.emotion, None);
        assert_eq!(e.clothes, None);
    }

    #[test]
    fn loose_bool_handles_each_shape() {
        assert_eq!(loose_bool(&json!(true)), Some(true));
        assert_eq!(loose_bool(&json!("TRUE")), Some(true));
        assert_eq!(loose_bool(&json!("false")), Some(false));
        assert_eq!(loose_bool(&json!("")), None);
        assert_eq!(loose_bool(&json!(null)), None);
    }
}
