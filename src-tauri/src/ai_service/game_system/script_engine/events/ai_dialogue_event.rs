//! AI dialogue event — sets character and generates an AI reply via MessageGenerator.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::ai_service::game_system::script_engine::events::{
    register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::generator::{GeneratorDeps, MessageGenerator};
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;
use crate::AppState;

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
        let script_status = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;

        let (role_id, _role_display_name) = {
            let mut gs = ctx.game_status.lock().await;
            let role = script_function::get_role(&mut *gs, ctx.db, &script_status, &self.character)
                .await?;
            let id = role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?;
            let dn = role.display_name.clone();
            (id, dn)
        };

        // Set as current character
        ctx.game_status.lock().await.current_role_id = Some(role_id);

        tracing::info!("[AIDialogueEvent] 开始执行");

        // Inject prompt as SYSTEM line if provided
        // TODO: 这里的 prompt 是暂时的，应该标记为临时 prompt，并且在代码逻辑中在AI回复后清除这部分提示词。
        if let Some(ref prompt) = self.prompt {
            let sys_line = LineBase {
                content: PromptRole::Plot.build_prompt(prompt),
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("旁白".to_string()),
                ..Default::default()
            };
            ctx.game_status
                .lock()
                .await
                .add_line(ctx.db, sys_line)
                .await?;
        }

        // Delegate AI response to MessageGenerator
        let state = ctx.app.state::<AppState>();
        let llm = match state.chat.llm.clone() {
            Some(llm) => llm,
            None => {
                tracing::warn!("[AIDialogueEvent] LLM 未配置，跳过 AI 对话");
                return Ok(None);
            }
        };

        let deps = GeneratorDeps {
            app: ctx.app.clone(),
            db: ctx.db.clone(),
            game_status: ctx.game_status.clone(),
            processor: state.chat.processor.clone(),
            translator: state.chat.translator.clone(),
            llm,
            concurrency: 1,
            god_agent: None,
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
