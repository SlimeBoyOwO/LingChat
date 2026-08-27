//! Force choice event — DDLC 式"鼠标被拖走"的强制选择。
//!
//! 与 `choices` 共用同一条 oneshot 通道和选项匹配逻辑，区别只在 payload
//! 多带一个 `forced` 字段：前端演出结束后只能提交这个选项的文本。

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    ForceChoiceGuard, ScriptContext, ScriptEvent, evaluate_condition, parse_duration,
    register_event,
};
use crate::ai_service::game_system::script_engine::responses::{
    ChoiceItem, ForceChoicePayload, event_names::SCRIPT_FORCE_CHOICE,
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
        let (vars, cursor_warp_allowed) = {
            let status = ctx.game_status.lock().await.script_status.clone();
            let allowed = !ctx.is_preview
                && status.as_ref().is_some_and(|script| {
                    script.content_warning.as_deref() == Some("horror")
                        && script
                            .settings
                            .get("allow_system_effects")
                            .and_then(Value::as_bool)
                            .unwrap_or(false)
                });
            (status.map(|script| script.vars), allowed)
        };

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

        // 强制项必须唯一、存在且未锁定；配置错误直接中止事件，绝不能让前端
        // 自动提交一个不存在的文本，或悄悄退化后执行非预期分支。
        let forced = self.forced.clone();
        let forced_matches = choices
            .iter()
            .filter(|choice| choice.text == forced && !choice.disabled)
            .count();
        if forced.is_empty() || forced_matches != 1 {
            return Err(anyhow!(
                "force_choice.forced 必须唯一匹配一个未锁定选项: '{}'",
                forced
            ));
        }
        let request_id = uuid::Uuid::new_v4().to_string();

        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut ch = ctx.channels.lock().await;
            ch.choice_tx = Some(tx);
            ch.choice_allow_free = false;
            ch.force_choice_guard = Some(ForceChoiceGuard {
                request_id: request_id.clone(),
                forced: forced.clone(),
                warp_enabled: cursor_warp_allowed,
                warp_expires_at: std::time::Instant::now() + std::time::Duration::from_secs(5),
            });
            rx
        };

        let payload = ForceChoicePayload {
            request_id: request_id.clone(),
            choices,
            forced: forced.clone(),
            duration: self.duration,
        };
        if let Err(error) = emit(ctx.app, SCRIPT_FORCE_CHOICE, &payload) {
            let mut channels = ctx.channels.lock().await;
            channels.choice_tx = None;
            channels.force_choice_guard = None;
            return Err(anyhow!("发送 force_choice 事件失败: {error}"));
        }

        let choice_result = rx.await;
        {
            let mut channels = ctx.channels.lock().await;
            channels.choice_allow_free = false;
            if channels
                .force_choice_guard
                .as_ref()
                .is_some_and(|guard| guard.request_id == request_id)
            {
                channels.force_choice_guard = None;
            }
        }
        let user_choice = choice_result.map_err(|_| anyhow!("用户选择通道已关闭"))?;
        if user_choice != forced {
            return Err(anyhow!("force_choice 后端拒绝非 forced 选项"));
        }

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
