//! 自由对话事件 —— 剧本内的多轮自由对话。
//!
//! 发出 free_dialogue 开始/结束边界，每轮等待输入，并把 AI 生成委托给 MessageGenerator。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::{SCRIPT_FREE_DIALOGUE, SCRIPT_INPUT},
    FreeDialoguePayload, InputPayload,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::generator::{
    GeneratorDeps, GeneratorSource, MessageGenerator,
};
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::{replace_placeholder, PromptRole};
use crate::AppState;

pub struct FreeDialogueEvent {
    character: String,
    hint: String,
    max_rounds: i32,
    end_line: String,
    dialog_prompt: String,
    end_prompt: String,
    duration: Option<f64>,
}

impl FreeDialogueEvent {
    fn from_event_data(data: &Value) -> Self {
        let dialog_prompt = data
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let end_prompt = data
            .get("end_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Self {
            // 与 dialogue / ai_dialogue / modify_character 对齐。
            // 原默认值是 "default"，而 get_role 只把字面量 "MAIN" 解析成主角，
            // 其余值一律去 DB 查 script_role_key —— 从来没有角色叫 "default"，
            // 所以省略 character 时必然报「角色 default 未在数据库中找到」。
            // 官方 5 处 free_dialogue 都显式写了 character，改默认值不影响它们。
            character: data
                .get("character")
                .and_then(|v| v.as_str())
                .unwrap_or("MAIN")
                .to_string(),
            hint: data
                .get("hint")
                .and_then(|v| v.as_str())
                .unwrap_or("自由对话...")
                .to_string(),
            max_rounds: data
                .get("max_rounds")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1) as i32,
            end_line: data
                .get("end_line")
                .and_then(|v| v.as_str())
                .unwrap_or("结束")
                .to_string(),
            dialog_prompt,
            end_prompt,
            duration: parse_duration(data),
        }
    }
}

#[async_trait]
impl ScriptEvent for FreeDialogueEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        // ---- 角色设置 ----
        let script_status = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;

        let role_id = {
            let mut gs = ctx.game_status.lock().await;
            let role = script_function::get_role(&mut *gs, ctx.db, &script_status, &self.character)
                .await?;
            role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?
        };

        // 设为当前角色
        ctx.game_status.lock().await.current_role_id = Some(role_id);

        // ---- 发送自由对话开始事件 ----
        let start_payload = FreeDialoguePayload {
            switch: true,
            max_rounds: self.max_rounds,
            end_line: self.end_line.clone(),
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_FREE_DIALOGUE, &start_payload);

        // ---- 构建 MessageGenerator（复用以提高性能） ----
        // LLM 未配置则直接终止剧本：自由对话需要 AI 判断何时收尾，没有 LLM 会
        // 陷入「玩家反复输入、永远收不了尾」的死循环（PR1 修的正是这个死锁），
        // 不能像现在这样静默跳过 AI 回复把剧本带偏（上游要求致命错误立即终止）。
        let generator = {
            let state = ctx.app.state::<AppState>();
            let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;
            let llm = llm.ok_or_else(|| {
                anyhow!("尚未配置大模型，无法执行「自由对话」事件，剧本终止。请先在设置里配置并选择模型。")
            })?;
            let deps = GeneratorDeps {
                source: GeneratorSource::ScriptFreeDialogue,
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
                // 试玩中跑的自由对话也属于试玩场次：捕获当前代号，迟到写入会被守卫丢弃
                generation: ctx.game_status.lock().await.preview_generation,
                is_preview: ctx.is_preview,
            };
            MessageGenerator::new(deps)
        };

        // ---- 替换 prompt 中的占位符 ----
        let game_status_guard = ctx.game_status.lock().await;
        let dialog_prompt = replace_placeholder(&self.dialog_prompt, &game_status_guard);
        let end_prompt = replace_placeholder(&self.end_prompt, &game_status_guard);
        drop(game_status_guard); // 尽早释放锁

        // ---- 主循环（支持无限轮次） ----
        let mut rounds: i32 = 0;
        loop {
            let mut is_last_round = false;

            rounds += 1;
            if self.max_rounds > 0 && rounds >= self.max_rounds {
                is_last_round = true;
            }

            tracing::info!(
                "[FreeDialogueEvent] 第 {} 轮 / {} 自由对话",
                rounds,
                if self.max_rounds > 0 {
                    self.max_rounds.to_string()
                } else {
                    "∞".into()
                }
            );

            // ---- 请求用户输入 ----
            let rx = {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let mut ch = ctx.channels.lock().await;
                ch.input_tx = Some(tx);
                rx
            };
            let payload = InputPayload {
                hint: self.hint.clone(),
                duration: self.duration,
            };
            let _ = emit(ctx.app, SCRIPT_INPUT, &payload);

            let user_input = rx.await.map_err(|_| anyhow!("用户输入通道已关闭"))?;

            // ---- 添加用户输入台词先 ----
            {
                let mut gs = ctx.game_status.lock().await;
                let line = crate::ai_service::types::LineBase {
                    content: user_input.clone(),
                    attribute: LineAttributeExt(LineAttribute::User),
                    display_name: Some(gs.player.user_name.clone()),
                    ..Default::default()
                };
                gs.add_line(ctx.db, line).await?;
            }

            // ---- 检查结束词（子串匹配） ----
            if !self.end_line.is_empty() && user_input.contains(&self.end_line) {
                is_last_round = true;
            }

            // ---- 构造带剧情提示的用户消息 ----
            let selected_prompt = if is_last_round {
                &end_prompt
            } else {
                &dialog_prompt
            };

            // ---- 添加系统旁白提示消息 ----
            // TODO: 这里的 prompt 是暂时的，应该标记为临时 prompt，并且在代码逻辑中在AI回复后清除这部分提示词。
            if !selected_prompt.is_empty() {
                let sys_line = LineBase {
                    content: PromptRole::Plot.build_prompt(selected_prompt),
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

            // ---- 调用 AI 生成回复 ----
            generator.process_message(None).await?;

            if is_last_round {
                break;
            }
        }

        // ---- 发送自由对话结束事件 ----
        let end_payload = FreeDialoguePayload {
            switch: false,
            max_rounds: self.max_rounds,
            end_line: self.end_line.clone(),
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_FREE_DIALOGUE, &end_payload);

        Ok(None)
    }

    fn event_type() -> &'static str {
        "free_dialogue"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(FreeDialogueEvent::event_type(), |data| {
        Box::new(FreeDialogueEvent::from_event_data(&data))
    });
}
