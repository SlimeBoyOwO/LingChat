//! Force choice event — DDLC 式"鼠标被拖走"的强制选择。
//!
//! 与 `choices` 共用同一条 oneshot 通道和选项匹配逻辑，区别只在 payload
//! 多带一个 `forced` 字段：前端演出结束后只能提交这个选项的文本。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    evaluate_condition, parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_FORCE_CHOICE, ChoiceItem, ForceChoicePayload,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;

pub struct ForceChoiceEvent {
    options: Vec<Value>,
    forced: String,
    duration: Option<f64>,
}

impl ForceChoiceEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            options: data
                .get("options")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            forced: data
                .get("forced")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for ForceChoiceEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let vars = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .map(|s| s.vars);

        // 与 ChoiceEvent 一致：条件不满足的选项标记 disabled + lock_hint
        let choices: Vec<ChoiceItem> = self
            .options
            .iter()
            .filter_map(|o| {
                let text = o.get("text").and_then(|v| v.as_str())?.to_string();
                let mut item = ChoiceItem {
                    text,
                    disabled: false,
                    reason: None,
                };
                if let Some(ref vars) = vars {
                    let condition = o.get("condition").and_then(|v| v.as_str()).unwrap_or("");
                    if !condition.is_empty() && !evaluate_condition(condition, vars) {
                        item.disabled = true;
                        item.reason = o
                            .get("lock_hint")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                    }
                }
                Some(item)
            })
            .collect();

        // forced 必须指向一个实际存在且未被锁定的选项，否则退化成普通 choices 行为：
        // 前端看不到有效强制项时会放任玩家自选，避免剧本卡死。
        let forced = self.forced.clone();
        if !forced.is_empty() && !choices.iter().any(|c| c.text == forced && !c.disabled) {
            tracing::warn!(
                "[ForceChoiceEvent] forced 选项 '{}' 不存在或被锁定，将不强制",
                forced
            );
        }

        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut ch = ctx.channels.lock().await;
            ch.choice_tx = Some(tx);
            ch.choice_allow_free = false;
            rx
        };

        let payload = ForceChoicePayload {
            choices,
            forced,
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_FORCE_CHOICE, &payload);

        let user_choice = rx.await.map_err(|_| anyhow!("用户选择通道已关闭"))?;
        ctx.channels.lock().await.choice_allow_free = false;

        tracing::info!("[ForceChoiceEvent] 用户选择(强制演出): {}", user_choice);

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

        ctx.game_status.lock().await.script_status = Some(script_status);

        if !matched {
            let mut gs = ctx.game_status.lock().await;
            let line = LineBase {
                content: user_choice,
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some(gs.player.user_name.clone()),
                sender_role_id: Some(0),
                ..Default::default()
            };
            gs.add_line(ctx.db, line).await?;
        }

        Ok(None)
    }

    fn event_type() -> &'static str {
        "force_choice"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(ForceChoiceEvent::event_type(), |data| {
        Box::new(ForceChoiceEvent::from_event_data(&data))
    });
}
