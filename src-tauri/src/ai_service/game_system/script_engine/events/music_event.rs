//! Music event — sets `game_status.background_music`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_MUSIC, MusicPayload,
};
use crate::ai_service::game_system::script_engine::utils::media::{resolve_script_media, MediaType};
use crate::ai_service::message_system::events::emit;

pub struct MusicEvent {
    music_path: String,
}

impl MusicEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            music_path: data
                .get("musicPath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }
}

#[async_trait]
impl ScriptEvent for MusicEvent {
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
            &self.music_path,
            MediaType::Music,
        )
        .unwrap_or_default();

        ctx.game_status.lock().await.background_music = resolved.clone();

        let payload = MusicPayload {
            music_path: resolved,
        };
        let _ = emit(ctx.app, SCRIPT_MUSIC, &payload);

        tracing::info!("[MusicEvent] BGM: {}", self.music_path);
        Ok(None)
    }

    fn event_type() -> &'static str {
        "music"
    }
}

pub fn register() {
    register_event(MusicEvent::event_type(), |data| {
        Box::new(MusicEvent::from_event_data(&data))
    });
}
