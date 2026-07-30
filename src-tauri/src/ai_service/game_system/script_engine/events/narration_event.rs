//! Narration event — displays narrator text and adds an ASSISTANT line.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_NARRATION, NarrationPayload,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;

pub struct NarrationEvent {
    text: String,
    display_name: Option<String>,
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
        }
    }
}

#[async_trait]
impl ScriptEvent for NarrationEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // Split text by newlines and emit each line separately
        let lines: Vec<&str> = self
            .text
            .split('\n')
            .filter(|line| !line.is_empty())
            .collect();

        for line in lines {
            let payload = NarrationPayload {
                text: line.to_string(),
                display_name: self.display_name.clone(),
            };
            let _ = emit(ctx.app, SCRIPT_NARRATION, &payload);
        }

        // Add as ASSISTANT line (keep the original text with newlines)
        let line = LineBase {
            content: self.text.clone(),
            attribute: LineAttributeExt(LineAttribute::Assistant),
            display_name: self.display_name.clone().or_else(|| Some("旁白".into())),
            ..Default::default()
        };
        ctx.game_status.lock().await.add_line(ctx.db, line).await?;

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
