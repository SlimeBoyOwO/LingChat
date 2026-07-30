//! Background effect event — sets `game_status.background_effect`.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_BACKGROUND_EFFECT, BackgroundEffectPayload,
};
use crate::ai_service::message_system::events::emit;

pub struct BackgroundEffectEvent {
    effect: String,
}

impl BackgroundEffectEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            effect: data
                .get("effect")
                .and_then(|v| v.as_str())
                .unwrap_or("none")
                .to_string(),
        }
    }
}

#[async_trait]
impl ScriptEvent for BackgroundEffectEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        ctx.game_status.lock().await.background_effect = self.effect.clone();

        let payload = BackgroundEffectPayload {
            effect: self.effect.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_BACKGROUND_EFFECT, &payload);

        Ok(None)
    }

    fn event_type() -> &'static str {
        "background_effect"
    }
}

pub fn register() {
    register_event(BackgroundEffectEvent::event_type(), |data| {
        Box::new(BackgroundEffectEvent::from_event_data(&data))
    });
}
