//! Sound effect event — emits a sound to the frontend without storing state.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_SOUND, SoundPayload,
};
use crate::ai_service::game_system::script_engine::utils::media::{
    resolve_script_media, MediaType,
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
        let script_path = ctx
            .game_status
            .lock()
            .await
            .script_status
            .as_ref()
            .map(|ss| ss.script_path.clone());

        let resolved = resolve_script_media(
            ctx.data_dir,
            script_path.as_deref(),
            &self.sound_path,
            MediaType::Sound,
        )
        .unwrap_or_default();

        let payload = SoundPayload {
            sound_path: resolved,
        };
        let _ = emit(ctx.app, SCRIPT_SOUND, &payload);

        tracing::info!("[SoundEvent] SFX: {}", self.sound_path);
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
