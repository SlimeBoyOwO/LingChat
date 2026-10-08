//! Jumpscare event — full-screen image flash + sound sting (horror stories).
//!
//! Fire-and-forget: the script continues to the next event immediately;
//! the frontend overlay hides itself after `duration` (default 0.6s).

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_JUMPSCARE, JumpscarePayload,
};
use crate::ai_service::game_system::script_engine::utils::media::{
    resolve_script_media, MediaType,
};
use crate::ai_service::message_system::events::emit;

pub struct JumpscareEvent {
    image_path: String,
    sound_path: String,
    duration: Option<f64>,
}

impl JumpscareEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            image_path: data
                .get("imagePath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            sound_path: data
                .get("soundPath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for JumpscareEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let script_path = ctx
            .game_status
            .lock()
            .await
            .script_status
            .as_ref()
            .map(|ss| ss.script_path.clone());

        let image = resolve_script_media(
            ctx.data_dir,
            script_path.as_deref(),
            &self.image_path,
            MediaType::Pic,
        )
        .unwrap_or_default();
        if image.is_empty() {
            tracing::warn!("[JumpscareEvent] 图片未找到: {}", self.image_path);
        }

        let sound = resolve_script_media(
            ctx.data_dir,
            script_path.as_deref(),
            &self.sound_path,
            MediaType::Sound,
        )
        .unwrap_or_default();

        let payload = JumpscarePayload {
            image_path: image,
            sound_path: sound,
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_JUMPSCARE, &payload);

        Ok(None)
    }

    fn event_type() -> &'static str {
        "jumpscare"
    }
}

pub fn register() {
    register_event(JumpscareEvent::event_type(), |data| {
        Box::new(JumpscareEvent::from_event_data(&data))
    });
}
