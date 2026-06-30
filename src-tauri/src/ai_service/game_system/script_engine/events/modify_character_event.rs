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
            perceive: data
                .get("perceive")
                .and_then(|v| v.as_str())
                .map(|s| s.eq_ignore_ascii_case("true")),
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
