//! Sound effect event — emits a sound to the frontend without storing state.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_SOUND, SoundPayload,
};
use crate::ai_service::message_system::events::emit;

pub struct SoundEvent {
    sound_path: String,
}

impl SoundEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            sound_path: data
                .get("soundPath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }
}

#[async_trait]
impl ScriptEvent for SoundEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let payload = SoundPayload {
            sound_path: self.sound_path.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_SOUND, &payload);

        log::info!("[SoundEvent] SFX: {}", self.sound_path);
        Ok(None)
    }

    fn event_type() -> &'static str {
        "sound"
    }
}

pub fn register() {
    register_event(SoundEvent::event_type(), |data| {
        Box::new(SoundEvent::from_event_data(&data))
    });
}
