//! AI 对话事件 —— 设定角色，并通过 MessageGenerator 生成 AI 回复。

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::AppState;
use crate::ai_service::game_system::game_status::HistorySession;
use crate::ai_service::game_system::script_engine::events::{
    ScriptContext, ScriptEvent, register_event,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::generator::{
    GeneratorDeps, GeneratorSource, MessageGenerator,
};
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

pub struct AIDialogueEvent {
    character: String,
    prompt: Option<String>,
}

impl AIDialogueEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            character: data
                .get("character")
                .and_then(|v| v.as_str())
                .unwrap_or("MAIN")
                .to_string(),
            prompt: data
                .get("prompt")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

#[async_trait]
impl ScriptEvent for AIDialogueEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // Capture the whole canonical-history identity at this event's first
        // relevant await. Do not later combine a newly read generation with
        // `ctx.is_preview`: Preview enter+restore can otherwise tear mode and
        // generation while role/LLM setup is in flight.
        let (script_status, request_session): (_, HistorySession) = {
            let gs = ctx.game_status.lock().await;
            (
                gs.script_status
                    .clone()
                    .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?,
                gs.history_session(),
            )
        };

        let role_id = {
            let mut gs = ctx.game_status.lock().await;
            let role = script_function::get_role(&mut *gs, ctx.db, &script_status, &self.character)
                .await?;
            role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?
        };

        // Set the role only while this event still owns the captured session;
        // a stale event must not update script state or start any downstream
        // generator/tool/marker/save work.
        {
            let mut gs = ctx.game_status.lock().await;
            if !gs.is_history_session_current(request_session) {
                return Err(anyhow!("AI 对话期间历史会话已切换，已丢弃本次事件"));
            }
            gs.current_role_id = Some(role_id);
        }

        tracing::info!("[AIDialogueEvent] 开始执行");

        // 若提供了 prompt，作为临时系统旁白台词注入
        // TODO: 这里的 prompt 是暂时的，应该标记为临时 prompt，并且在代码逻辑中在AI回复后清除这部分提示词。
        if let Some(ref prompt) = self.prompt {
            let sys_line = LineBase {
                content: PromptRole::Plot.build_prompt(prompt),
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("旁白".to_string()),
                ..Default::default()
            };
            let mut gs = ctx.game_status.lock().await;
            if !gs
                .append_line_if_current(ctx.db, request_session, sys_line)
                .await?
            {
                return Err(anyhow!("AI 对话期间历史会话已切换，已丢弃本次事件"));
            }
        }

        // 委托 MessageGenerator 生成回复
        let state = ctx.app.state::<AppState>();
        let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;
        let llm = match llm {
            Some(llm) => llm,
            None => {
                // LLM 未配置：AI 对话事件无法生成。按上游要求直接终止剧本，
                // 不再 fallback 到任何占位/默认文本——那会让剧本以错误逻辑继续跑。
                return Err(anyhow!(
                    "尚未配置大模型，无法执行「AI 对话」事件，剧本终止。请先在设置里配置并选择模型。"
                ));
            },
        };

        // `slot_snapshot` awaits. Atomically recheck the original identity
        // before constructing/admitting the generator pipeline.
        if !ctx
            .game_status
            .lock()
            .await
            .is_history_session_current(request_session)
        {
            return Err(anyhow!("AI 对话期间历史会话已切换，已丢弃本次事件"));
        }

        let deps = GeneratorDeps {
            source: GeneratorSource::ScriptAiDialogue,
            app: ctx.app.clone(),
            db: ctx.db.clone(),
            game_status: ctx.game_status.clone(),
            processor: state.chat.processor.clone(),
            translator: state.chat.translator.clone(),
            llm,
            tool_registry: state.tool_registry.clone(),
            concurrency: 1,
            god_agent: None,
            suppress_thinking: false,
            session: request_session,
        };

        let generator = MessageGenerator::new(deps);
        generator.process_message(None).await?;

        tracing::info!("[AIDialogueEvent] 执行完毕");

        Ok(None)
    }

    fn event_type() -> &'static str {
        "ai_dialogue"
    }
}

pub fn register() {
    register_event(AIDialogueEvent::event_type(), |data| {
        Box::new(AIDialogueEvent::from_event_data(&data))
    });
}
