//! Narration event — displays narrator text line by line and adds one ASSISTANT line per line.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    ScriptContext, ScriptEvent, parse_duration, register_event, split_script_text_lines,
};
use crate::ai_service::game_system::script_engine::responses::{
    NarrationPayload, event_names::SCRIPT_NARRATION,
};
use crate::ai_service::game_system::status_ops::PromptLine;
use crate::ai_service::message_system::events::emit;
use crate::utils::prompt::PromptRole;

pub struct NarrationEvent {
    text: String,
    display_name: Option<String>,
    duration: Option<f64>,
}

impl NarrationEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            text: data
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            display_name: data
                .get("displayName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for NarrationEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        for line in split_script_text_lines(&self.text) {
            let payload = NarrationPayload {
                text: line.to_string(),
                display_name: self.display_name.clone(),
                duration: self.duration,
            };
            let _ = emit(ctx.app, SCRIPT_NARRATION, &payload);

            // 逐行写入台词表，否则历史记录与读档后整段旁白会并成一句
            let mut prompt_line = PromptLine::new(PromptRole::Narrator, line).sender(Some(0));
            if let Some(name) = self.display_name.clone() {
                prompt_line = prompt_line.display_name(name);
            }
            ctx.game_status
                .lock()
                .await
                .add_line(ctx.db, prompt_line.into_base())
                .await?;
        }

        Ok(None)
    }

    fn event_type() -> &'static str {
        "narration"
    }
}

pub fn register() {
    register_event(NarrationEvent::event_type(), |data| {
        Box::new(NarrationEvent::from_event_data(&data))
    });
}
