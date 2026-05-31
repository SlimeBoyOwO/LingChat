//! Dialogue event — sets current_character and emits character dialogue lines.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::responses::ReplyResponse;
use crate::ai_service::types::{LineBase, LineAttributeExt};
use crate::db::entities::line::LineAttribute;

pub struct DialogueEvent {
    character: String,
    text: String,
    display_name: Option<String>,
    display_subtitle: Option<String>,
    emotion: Option<String>,
}

impl DialogueEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            character: data
                .get("character")
                .and_then(|v| v.as_str())
                .unwrap_or("MAIN")
                .to_string(),
            text: data
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            display_name: data
                .get("displayName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            display_subtitle: data
                .get("displaySubtitle")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            emotion: data
                .get("emotion")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl ScriptEvent for DialogueEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let script_status = ctx
            .game_status
            .lock().await
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;

        let (role_id, role_display_name) = {
            let mut gs = ctx.game_status.lock().await;
            let role = script_function::get_role(
                &mut *gs,
                ctx.db,
                &script_status,
                &self.character,
            )
            .await?;
            let id = role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?;
            let dn = role.display_name.clone();
            (id, dn)
        };

        // Now safe to mutate game_status
        ctx.game_status.lock().await.current_role_id = Some(role_id);

        // Get display info
        let display_name = self
            .display_name
            .clone()
            .or(role_display_name)
            .unwrap_or_default();
        let display_subtitle = self.display_subtitle.clone().unwrap_or_default();
        let emotion = self.emotion.clone().unwrap_or_default();

        // Emit via ai:reply (reuses the existing dialogue event processor)
        let payload = ReplyResponse {
            type_: "reply".to_string(),
            duration: -1.0,
            is_final: true,
            character: Some(self.character.clone()),
            role_id: Some(role_id),
            emotion: emotion.clone(),
            original_tag: String::new(),
            message: self.text.clone(),
            tts_text: None,
            motion_text: None,
            audio_file: None,
            original_message: self.text.clone(),
            display_name: Some(display_name.clone()),
            display_subtitle: Some(display_subtitle),
            user_message_seq: None,
        };
        let _ = emit(ctx.app, "ai:reply", &payload);

        // Add ASSISTANT line
        let line = LineBase {
            content: self.text.clone(),
            attribute: LineAttributeExt(LineAttribute::Assistant),
            sender_role_id: Some(role_id),
            display_name: Some(display_name),
            original_emotion: Some(emotion),
            ..Default::default()
        };
        ctx.game_status.lock().await.add_line(ctx.db, line).await?;

        Ok(None)
    }

    fn event_type() -> &'static str {
        "dialogue"
    }
}

pub fn register() {
    register_event(DialogueEvent::event_type(), |data| {
        Box::new(DialogueEvent::from_event_data(&data))
    });
}
