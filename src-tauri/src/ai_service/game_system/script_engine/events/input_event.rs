//! Input event — prompts the user for text input, waits, then adds as USER line.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{register_event, ScriptContext, ScriptEvent};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_INPUT, InputPayload,
};
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{LineBase, LineAttributeExt};
use crate::db::entities::line::LineAttribute;

pub struct InputEvent {
    hint: String,
}

impl InputEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            hint: data
                .get("hint")
                .and_then(|v| v.as_str())
                .unwrap_or("请输入...")
                .to_string(),
        }
    }
}

#[async_trait]
impl ScriptEvent for InputEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // Set up oneshot channel and store sender in shared channels (brief lock)
        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut ch = ctx.channels.lock().await;
            ch.input_tx = Some(tx);
            rx
        };

        // Emit input event to frontend
        let payload = InputPayload {
            hint: self.hint.clone(),
        };
        let _ = emit(ctx.app, SCRIPT_INPUT, &payload);

        // Await user input — no locks held
        let user_input = rx.await.map_err(|_| anyhow!("用户输入通道已关闭"))?;

        tracing::info!("[InputEvent] 收到用户输入: {}", user_input);

        // Add USER line
        let line = LineBase {
            content: user_input,
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(ctx.game_status.lock().await.player.user_name.clone()),
            sender_role_id: ctx.game_status.lock().await.main_role_id,
            ..Default::default()
        };
        ctx.game_status.lock().await.add_line(ctx.db, line).await?;

        Ok(None)
    }

    fn event_type() -> &'static str {
        "input"
    }
}

pub fn register() {
    register_event(InputEvent::event_type(), |data| {
        Box::new(InputEvent::from_event_data(&data))
    });
}
