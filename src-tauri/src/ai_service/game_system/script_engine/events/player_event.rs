//! 玩家事件 —— 展示玩家台词并写入一条 USER 台词行。

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    ScriptContext, ScriptEvent, parse_duration, register_event,
};
use crate::ai_service::game_system::script_engine::responses::{
    PlayerPayload, event_names::SCRIPT_PLAYER,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;

pub struct PlayerEvent {
    text: String,
    display_name: Option<String>,
    duration: Option<f64>,
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
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for PlayerEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 一次加锁取出玩家名（缓存）与当前附身实体：剧本玩家台词归属当前附身身份
        let (player_name, possessed_role_id) = {
            let gs = ctx.game_status.lock().await;
            (gs.player.user_name.clone(), gs.possessed_role_id)
        };
        let display_name = self.display_name.clone().unwrap_or(player_name);

        let payload = PlayerPayload {
            text: self.text.clone(),
            display_name: Some(display_name.clone()),
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_PLAYER, &payload);

        let line = LineBase {
            content: self.text.clone(),
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(display_name),
            sender_role_id: Some(possessed_role_id),
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
