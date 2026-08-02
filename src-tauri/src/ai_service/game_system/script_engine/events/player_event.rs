//! Player event — displays player text and adds a USER line.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_PLAYER, PlayerPayload,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;

pub struct PlayerEvent {
    text: String,
    display_name: Option<String>,
}

impl PlayerEvent {
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
impl ScriptEvent for PlayerEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let player_name = ctx.game_status.lock().await.player.user_name.clone();
        let display_name = self.display_name.clone().unwrap_or(player_name);

        let payload = PlayerPayload {
            text: self.text.clone(),
            display_name: Some(display_name.clone()),
        };
        let _ = emit(ctx.app, SCRIPT_PLAYER, &payload);

        let line = LineBase {
            content: self.text.clone(),
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(display_name),
            // 玩家台词一律标 sender_role_id=0（玩家），与 handle_user_message 对齐
            sender_role_id: Some(0),
            ..Default::default()
        };
        ctx.game_status.lock().await.add_line(ctx.db, line).await?;

        Ok(None)
    }

    fn event_type() -> &'static str {
        "player"
    }
}

pub fn register() {
    register_event(PlayerEvent::event_type(), |data| {
        Box::new(PlayerEvent::from_event_data(&data))
    });
}
