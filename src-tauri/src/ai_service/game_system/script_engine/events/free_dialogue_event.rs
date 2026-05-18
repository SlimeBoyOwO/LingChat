//! Free dialogue event — multi-round free conversation within a script.
//!
//! Emits free_dialogue start/stop boundaries, waits for input each round,
//! and generates AI responses. LLM integration is stubbed.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::{SCRIPT_FREE_DIALOGUE, SCRIPT_INPUT},
    FreeDialoguePayload, InputPayload,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::responses::{ReplyResponse, ThinkingResponse};
use crate::ai_service::types::{LineBase, LineAttributeExt};
use crate::db::entities::line::LineAttribute;

pub struct FreeDialogueEvent {
    hint: String,
    max_rounds: i32,
    end_line: String,
    prompt: Option<String>,
    end_prompt: Option<String>,
}

impl FreeDialogueEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            hint: data
                .get("hint")
                .and_then(|v| v.as_str())
                .unwrap_or("自由对话...")
                .to_string(),
            max_rounds: data
                .get("max_rounds")
                .and_then(|v| v.as_i64())
                .unwrap_or(5) as i32,
            end_line: data
                .get("end_line")
                .and_then(|v| v.as_str())
                .unwrap_or("结束")
                .to_string(),
            prompt: data
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            end_prompt: data
                .get("end_prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl ScriptEvent for FreeDialogueEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // Emit free_dialogue start
        let start_payload = FreeDialoguePayload {
            switch: true,
            max_rounds: self.max_rounds,
            end_line: self.end_line.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_FREE_DIALOGUE, &start_payload);

        for round in 1..=self.max_rounds {
            log::info!(
                "[FreeDialogueEvent] 第 {} 轮 / {} 自由对话",
                round,
                self.max_rounds
            );

            // Set up oneshot and emit input event (brief lock for channel setup)
            let rx = {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let mut ch = ctx.channels.lock().await;
                ch.input_tx = Some(tx);
                rx
            };

            let payload = InputPayload {
                hint: self.hint.clone(),
            };
            let _ = emit(ctx.app, SCRIPT_INPUT, &payload);

            // Await user input — no locks held
            let user_input = rx
                .await
                .map_err(|_| anyhow!("用户输入通道已关闭"))?;

            // Check if user wants to end
            if !self.end_line.is_empty()
                && user_input.trim() == self.end_line.trim()
            {
                log::info!("[FreeDialogueEvent] 用户触发结束词，退出自由对话");
                break;
            }

            // Add USER line
            let user_line = LineBase {
                content: user_input,
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some(ctx.game_status.player.user_name.clone()),
                sender_role_id: ctx.game_status.main_role_id,
                ..Default::default()
            };
            ctx.game_status.add_line(ctx.db, user_line).await?;

            // TODO: Full LLM integration — call AI and stream response
            // For now, emit placeholder
            let think = ThinkingResponse::new(true);
            let _ = emit(ctx.app, "ai:thinking", &think);

            let placeholder = format!(
                "[自由对话 第{}轮] AI 回复中...（LLM 集成尚未完成）",
                round
            );

            let reply = ReplyResponse {
                type_: "reply".to_string(),
                duration: -1.0,
                is_final: true,
                character: None,
                role_id: ctx.game_status.current_role_id,
                emotion: String::new(),
                original_tag: String::new(),
                message: placeholder.clone(),
                tts_text: None,
                motion_text: None,
                audio_file: None,
                original_message: placeholder.clone(),
                display_name: None,
                display_subtitle: None,
            };
            let _ = emit(ctx.app, "ai:reply", &reply);

            let think_off = ThinkingResponse::new(false);
            let _ = emit(ctx.app, "ai:thinking", &think_off);

            // Add ASSISTANT line
            let ai_line = LineBase {
                content: placeholder,
                attribute: LineAttributeExt(LineAttribute::Assistant),
                sender_role_id: ctx.game_status.current_role_id,
                ..Default::default()
            };
            ctx.game_status.add_line(ctx.db, ai_line).await?;
        }

        // Emit free_dialogue end
        let end_payload = FreeDialoguePayload {
            switch: false,
            max_rounds: self.max_rounds,
            end_line: self.end_line.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_FREE_DIALOGUE, &end_payload);

        Ok(None)
    }

    fn event_type() -> &'static str {
        "free_dialogue"
    }
}

pub fn register() {
    register_event(FreeDialogueEvent::event_type(), |data| {
        Box::new(FreeDialogueEvent::from_event_data(&data))
    });
}
