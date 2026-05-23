//! Background event — sets `game_status.background` and emits to frontend.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_BACKGROUND, BackgroundPayload,
};
use crate::ai_service::message_system::events::emit;

pub struct BackgroundEvent {
    image_path: String,
    transition: f64,
}

impl BackgroundEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            image_path: data
                .get("imagePath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            transition: data
                .get("transition")
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0),
        }
    }
}

#[async_trait]
impl ScriptEvent for BackgroundEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        ctx.game_status.lock().await.background = self.image_path.clone();

        let payload = BackgroundPayload {
            image_path: self.image_path.clone(),
            transition: self.transition,
        };
        let _ = emit(ctx.app, SCRIPT_BACKGROUND, &payload);

        log::info!("[BackgroundEvent] 背景切换: {}", self.image_path);
        Ok(None)
    }

    fn event_type() -> &'static str {
        "background"
    }
}

/// Register on module load
pub fn register() {
    register_event(BackgroundEvent::event_type(), |data| {
        Box::new(BackgroundEvent::from_event_data(&data))
    });
}
