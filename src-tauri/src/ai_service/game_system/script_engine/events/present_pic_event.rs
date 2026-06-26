//! Present picture event — shows a full-screen image with optional scale.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_PRESENT_PIC, PresentPicPayload,
};
use crate::ai_service::game_system::script_engine::utils::media::{
    resolve_script_media, MediaType,
};
use crate::ai_service::message_system::events::emit;

pub struct PresentPicEvent {
    image_path: String,
    scale: f64,
}

impl PresentPicEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            image_path: data
                .get("imagePath")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            scale: data.get("scale").and_then(|v| v.as_f64()).unwrap_or(1.0),
        }
    }
}

#[async_trait]
impl ScriptEvent for PresentPicEvent {
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
            &self.image_path,
            MediaType::Pic,
        )
        .unwrap_or_default();

        ctx.game_status.lock().await.present_pic = resolved.clone();

        let payload = PresentPicPayload {
            image_path: resolved,
            scale: self.scale,
        };
        let _ = emit(ctx.app, SCRIPT_PRESENT_PIC, &payload);

        Ok(None)
    }

    fn event_type() -> &'static str {
        "present_pic"
    }
}

pub fn register() {
    register_event(PresentPicEvent::event_type(), |data| {
        Box::new(PresentPicEvent::from_event_data(&data))
    });
}
