//! Free dialogue event — multi-round free conversation within a script.
//!
//! Emits free_dialogue start/stop boundaries, waits for input each round,
//! and delegates AI generation to MessageGenerator.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::{SCRIPT_FREE_DIALOGUE, SCRIPT_INPUT},
    FreeDialoguePayload, InputPayload,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::generator::{GeneratorDeps, MessageGenerator};
use crate::AppState;

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

        // Build MessageGenerator once (reused across rounds)
        let generator = {
            let state = ctx.app.state::<AppState>();
            state.chat.llm.clone().map(|llm| {
                let deps = GeneratorDeps {
                    app: ctx.app.clone(),
                    db: ctx.db.clone(),
                    game_status: ctx.game_status.clone(),
                    processor: state.chat.processor.clone(),
                    translator: state.chat.translator.clone(),
                    llm,
                    concurrency: 1,
                };
                MessageGenerator::new(deps)
            })
        };

        for round in 1..=self.max_rounds {
            log::info!(
                "[FreeDialogueEvent] 第 {} 轮 / {} 自由对话",
                round,
                self.max_rounds
            );

            // Set up oneshot and emit input event
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

            // Delegate to MessageGenerator (handles USER line, LLM streaming, ASSISTANT line)
            if let Some(ref generator) = generator {
                generator.process_message(Some(user_input)).await?;
            } else {
                log::warn!("[FreeDialogueEvent] LLM 未配置，跳过 AI 回复");
            }
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
