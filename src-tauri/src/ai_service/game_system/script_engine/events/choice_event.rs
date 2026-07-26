//! Choice event — presents branching options to the user, waits for selection,
//! then evaluates conditions and executes actions for the matched option.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_CHOICE, ChoicePayload,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;

pub struct ChoiceEvent {
    options: Vec<Value>,
    allow_free: bool,
}

impl ChoiceEvent {
    fn from_event_data(data: &Value) -> Self {
        let options: Vec<Value> = data
            .get("options")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // 选项级 `next` 不存在：只有 chapter_end 的 options 支持 next。
        // 作者写了不会报错也不会生效，跳转直接失效，属于最容易踩的静默失败之一。
        for (i, opt) in options.iter().enumerate() {
            if let Some(next) = opt.get("next").and_then(|v| v.as_str()) {
                tracing::warn!(
                    "[ChoiceEvent] 第 {} 个选项写了 next: '{}'，但 choices 不支持选项级跳转，\
                     该字段被忽略；请改用 set_var + chapter_end 的 branching",
                    i + 1,
                    next
                );
            }
        }

        Self {
            options,
            allow_free: data
                .get("allow_free")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        }
    }
}

#[async_trait]
impl ScriptEvent for ChoiceEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // Build choice labels
        let choices: Vec<String> = self
            .options
            .iter()
            .filter_map(|o| o.get("text").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .collect();

        // Set up oneshot channel and store sender (brief lock).
        // `choice_allow_free` lets `script_submit_input` route free-typed text here
        // instead of rejecting it — without it an `allow_free: true` choice can
        // never be resolved and the script blocks forever.
        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut ch = ctx.channels.lock().await;
            ch.choice_tx = Some(tx);
            ch.choice_allow_free = self.allow_free;
            rx
        };

        // Emit choice event to frontend
        let payload = ChoicePayload {
            choices: choices.clone(),
            allow_free: self.allow_free,
        };
        let _ = emit(ctx.app, SCRIPT_CHOICE, &payload);

        // Await user choice — no locks held
        let user_choice = rx.await.map_err(|_| anyhow!("用户选择通道已关闭"))?;

        // The choice is resolved; stop accepting free text on its behalf.
        ctx.channels.lock().await.choice_allow_free = false;

        tracing::info!("[ChoiceEvent] 用户选择: {}", user_choice);

        // Clone out script_status to avoid double borrow
        let mut script_status = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;

        let matched = {
            let mut gs = ctx.game_status.lock().await;
            script_function::process_options(
                &mut *gs,
                ctx.db,
                &mut script_status,
                &self.options,
                Some(&user_choice),
            )
            .await?
        };

        // Write back potentially modified script_status
        ctx.game_status.lock().await.script_status = Some(script_status);

        if !matched {
            // Add raw input as USER line if no option matched
            let mut gs = ctx.game_status.lock().await;
            let line = LineBase {
                content: user_choice,
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some(gs.player.user_name.clone()),
                sender_role_id: gs.main_role_id,
                ..Default::default()
            };
            gs.add_line(ctx.db, line).await?;
        }

        Ok(None)
    }

    fn event_type() -> &'static str {
        "choices"
    }
}

pub fn register() {
    register_event(ChoiceEvent::event_type(), |data| {
        Box::new(ChoiceEvent::from_event_data(&data))
    });
}
