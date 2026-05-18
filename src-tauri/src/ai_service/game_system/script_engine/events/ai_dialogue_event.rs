//! AI dialogue event — sets character and streams an AI-generated reply.
//!
//! LLM integration is stubbed — currently logs a warning and emits placeholder text.
//! Full implementation requires wiring into the MessageGenerator / LLM pipeline.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::responses::{ReplyResponse, ThinkingResponse};
use crate::ai_service::types::{LineBase, LineAttributeExt};
use crate::db::entities::line::LineAttribute;

pub struct AIDialogueEvent {
    character: String,
    prompt: Option<String>,
}

impl AIDialogueEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            character: data
                .get("character")
                .and_then(|v| v.as_str())
                .unwrap_or("MAIN")
                .to_string(),
            prompt: data
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl ScriptEvent for AIDialogueEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let script_status = ctx
            .game_status
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;

        let (role_id, role_display_name) = {
            let role = script_function::get_role(
                ctx.game_status,
                ctx.db,
                &script_status,
                &self.character,
            )
            .await?;
            let id = role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?;
            let dn = role.display_name.clone();
            (id, dn)
        };

        // Set as current character
        ctx.game_status.current_role_id = Some(role_id);

        // Inject prompt as SYSTEM line if provided
        if let Some(ref prompt) = self.prompt {
            let sys_line = LineBase {
                content: prompt.clone(),
                attribute: LineAttributeExt(LineAttribute::System),
                sender_role_id: Some(role_id),
                display_name: role_display_name.clone(),
                ..Default::default()
            };
            ctx.game_status.add_line(ctx.db, sys_line).await?;
        }

        // TODO: Full LLM integration — stream AI response via MessageGenerator
        // For now, emit thinking → placeholder reply
        let think = ThinkingResponse::new(true);
        let _ = emit(ctx.app, "ai:thinking", &think);

        let display_name = role_display_name.clone().unwrap_or_else(|| "角色".to_string());
        let placeholder = format!(
            "[AI 对话] {} 正在思考中...（LLM 集成尚未完成）",
            display_name
        );

        let reply = ReplyResponse {
            type_: "reply".to_string(),
            duration: -1.0,
            is_final: true,
            character: Some(self.character.clone()),
            role_id: Some(role_id),
            emotion: String::new(),
            original_tag: String::new(),
            message: placeholder.clone(),
            tts_text: None,
            motion_text: None,
            audio_file: None,
            original_message: placeholder.clone(),
            display_name: role_display_name.clone(),
            display_subtitle: None,
        };
        let _ = emit(ctx.app, "ai:reply", &reply);

        let think_off = ThinkingResponse::new(false);
        let _ = emit(ctx.app, "ai:thinking", &think_off);

        // Add ASSISTANT line
        let line = LineBase {
            content: placeholder,
            attribute: LineAttributeExt(LineAttribute::Assistant),
            sender_role_id: Some(role_id),
            display_name: role_display_name.clone(),
            ..Default::default()
        };
        ctx.game_status.add_line(ctx.db, line).await?;

        Ok(None)
    }

    fn event_type() -> &'static str {
        "ai_dialogue"
    }
}

pub fn register() {
    register_event(AIDialogueEvent::event_type(), |data| {
        Box::new(AIDialogueEvent::from_event_data(&data))
    });
}
