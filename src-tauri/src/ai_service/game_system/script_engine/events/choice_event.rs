//! 选项事件 —— 向用户展示分支选项，等待选择后，
//! 对匹配的选项求值条件并执行其动作。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    evaluate_condition, parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_CHOICE, ChoiceItem, ChoicePayload,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;

pub struct ChoiceEvent {
    options: Vec<Value>,
    allow_free: bool,
    duration: Option<f64>,
}

impl ChoiceEvent {
    fn from_event_data(data: &Value) -> Self {
        let options: Vec<Value> = data
            .get("options")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        Self {
            options,
            allow_free: data
                .get("allow_free")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for ChoiceEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // 读取当前变量，用于逐项判断选项是否因条件不满足而锁定（短暂持锁）。
        // 与 process_options 的 continue 逻辑保持一致：条件不满足的选项永远不被匹配。
        let vars = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .map(|s| s.vars);

        // 构建选项列表：条件不满足的选项标记 disabled，并带上作者写的 lock_hint。
        // 没有文案的兜底选项不会发给前端（玩家看不到，仅后端匹配用），与旧行为一致。
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
                // 只有 script_status 存在（剧本运行中）才可能求值条件；早期事件没有它
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

        // 建立 oneshot 通道并存入 sender（短暂持锁）。
        // `choice_allow_free` 让 `script_submit_input` 把自由输入的文本转投到这里，
        // 而不是拒绝——否则 `allow_free: true` 的选项永远无法解决，剧本永久阻塞。
        let rx = {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let mut ch = ctx.channels.lock().await;
            ch.choice_tx = Some(tx);
            ch.choice_allow_free = self.allow_free;
            rx
        };

        // 向前端发出选项事件
        let payload = ChoicePayload {
            choices,
            allow_free: self.allow_free,
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_CHOICE, &payload);

        // 等待用户选择——不持有任何锁
        let user_choice = rx.await.map_err(|_| anyhow!("用户选择通道已关闭"))?;

        // 选项已解决；停止替它接受自由输入。
        ctx.channels.lock().await.choice_allow_free = false;

        tracing::info!("[ChoiceEvent] 用户选择: {}", user_choice);

        // 克隆出 script_status 以避免双重借用
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

        // 写回可能被修改的 script_status
        ctx.game_status.lock().await.script_status = Some(script_status);

        if !matched {
            // 没有选项命中时，把原始输入作为 USER 台词写入
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
        "choices"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(ChoiceEvent::event_type(), |data| {
        Box::new(ChoiceEvent::from_event_data(&data))
    });
}
