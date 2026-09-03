//! Set player identity event — 在剧本运行中切换玩家身份（叙事/对话视角）。
//!
//! 解耦玩家与 AI 设定后的剧本级身份切换机制。剧本作者可在章节中临时把玩家
//! 变成「另一个人」（如视角切换），支持：
//! - `persona_id`（可选）：以哪张玩家人设卡为基底（chapter/script 生效；permanent 忽略）
//! - `user_name` / `user_subtitle` / `user_prompt`：新的玩家身份字段（均可选）
//! - `scope`：`"chapter"`（默认，章节结束后还原）/ `"script"`（剧本结束后还原）/
//!   `"permanent"`（写入全局玩家档案，永久生效）
//!
//! 状态保存与还原统一走 `game_system::player_profile_sync`：快照栈记录
//! 原身份 + scope，chapter_end / on_script_end 按作用域弹出。

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::ai_service::game_system::player_profile_sync::apply_player_identity;
use crate::ai_service::game_system::script_engine::events::{
    ScriptContext, ScriptEvent, register_event,
};
use crate::ai_service::types::IdentityScope;

pub struct SetPlayerIdentityEvent {
    persona_id: Option<String>,
    user_name: Option<String>,
    user_subtitle: Option<String>,
    user_prompt: Option<String>,
    scope: IdentityScope,
}

impl SetPlayerIdentityEvent {
    fn from_event_data(data: &Value) -> Self {
        let scope_raw = data
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("chapter");
        Self {
            persona_id: data
                .get("persona_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            user_name: data
                .get("user_name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            user_subtitle: data
                .get("user_subtitle")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            user_prompt: data
                .get("user_prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            scope: IdentityScope::parse(scope_raw),
        }
    }
}

#[async_trait]
impl ScriptEvent for SetPlayerIdentityEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let persona_id = self.persona_id.take();
        apply_player_identity(
            ctx,
            persona_id.clone(),
            self.user_name.take(),
            self.user_subtitle.take(),
            self.user_prompt.take(),
            self.scope,
        )
        .await?;

        tracing::info!(
            "[SetPlayerIdentityEvent] 玩家身份切换完成 (scope={}, persona_id={})",
            self.scope.as_str(),
            persona_id.as_deref().unwrap_or("<none>"),
        );

        Ok(None)
    }

    fn event_type() -> &'static str {
        "set_player_identity"
    }
}

pub fn register() {
    register_event(SetPlayerIdentityEvent::event_type(), |data| {
        Box::new(SetPlayerIdentityEvent::from_event_data(&data))
    });
}
