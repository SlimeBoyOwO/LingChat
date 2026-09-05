//! 自由对话事件 —— 剧本内的多轮自由对话。
//!
//! 发出 free_dialogue 开始/结束边界，每轮等待输入，并把 AI 生成委托给 MessageGenerator。

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::AppState;
use crate::ai_service::game_system::game_status::{GameStatus, HistorySession};
use crate::ai_service::game_system::script_engine::events::{
    ScriptContext, ScriptEvent, parse_duration, register_event,
};
use crate::ai_service::game_system::script_engine::responses::{
    FreeDialoguePayload, InputPayload,
    event_names::{SCRIPT_FREE_DIALOGUE, SCRIPT_INPUT},
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::generator::{
    GeneratorDeps, GeneratorSource, MessageGenerator,
};
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::{PromptRole, replace_placeholder};

pub struct FreeDialogueEvent {
    character: String,
    hint: String,
    max_rounds: i32,
    end_line: String,
    dialog_prompt: String,
    end_prompt: String,
    duration: Option<f64>,
}

fn stale_history_session_error() -> anyhow::Error {
    anyhow!("自由对话期间历史会话已切换，已丢弃本次事件")
}

async fn ensure_history_session_current(
    game_status: &std::sync::Arc<tokio::sync::Mutex<GameStatus>>,
    request_session: HistorySession,
) -> Result<()> {
    game_status
        .lock()
        .await
        .is_history_session_current(request_session)
        .then_some(())
        .ok_or_else(stale_history_session_error)
}

async fn append_free_dialogue_line_if_current(
    game_status: &std::sync::Arc<tokio::sync::Mutex<GameStatus>>,
    db: &sea_orm::DatabaseConnection,
    request_session: HistorySession,
    line: LineBase,
) -> Result<()> {
    if game_status
        .lock()
        .await
        .append_line_if_current(db, request_session, line)
        .await?
    {
        Ok(())
    } else {
        Err(stale_history_session_error())
    }
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
        // Capture the complete canonical-history identity at this event's
        // first await. Every later await (role, LLM, input setup/input wait)
        // must retain this one identity; never combine a newer generation with
        // the old event's preview mode.
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

        // Role lookup awaited. A stale event must not mutate script state or
        // announce/start any downstream work.
        {
            let mut gs = ctx.game_status.lock().await;
            if !gs.is_history_session_current(request_session) {
                return Err(anyhow!("自由对话期间历史会话已切换，已丢弃本次事件"));
            }
            gs.current_role_id = Some(role_id);
        }

        // ---- 构建 MessageGenerator（复用以提高性能） ----
        // LLM 未配置则直接终止剧本：自由对话需要 AI 判断何时收尾，没有 LLM 会
        // 陷入「玩家反复输入、永远收不了尾」的死循环（PR1 修的正是这个死锁），
        // 不能像现在这样静默跳过 AI 回复把剧本带偏（上游要求致命错误立即终止）。
        let generator = {
            let state = ctx.app.state::<AppState>();
            let llm = crate::ai_service::llm::slot_snapshot(&state.chat.llm).await;
            ensure_history_session_current(&ctx.game_status, request_session).await?;
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
                session: request_session,
            };
            MessageGenerator::new(deps)
        };

        // ---- 替换 prompt 中的占位符 ----
        let game_status_guard = ctx.game_status.lock().await;
        let dialog_prompt = replace_placeholder(&self.dialog_prompt, &game_status_guard);
        let end_prompt = replace_placeholder(&self.end_prompt, &game_status_guard);
        drop(game_status_guard); // 尽早释放锁
        ensure_history_session_current(&ctx.game_status, request_session).await?;

        // The event becomes externally visible only after all asynchronous
        // setup has retained the captured session.
        let start_payload = FreeDialoguePayload {
            switch: true,
            max_rounds: self.max_rounds,
            end_line: self.end_line.clone(),
            duration: self.duration,
        };
        let _ = emit(ctx.app, SCRIPT_FREE_DIALOGUE, &start_payload);

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
            // Channel setup awaited. If Preview changed while it was pending,
            // do not publish an input request or leave this old event alive.
            if let Err(error) =
                ensure_history_session_current(&ctx.game_status, request_session).await
            {
                ctx.channels.lock().await.input_tx = None;
                return Err(error);
            }
            let payload = InputPayload {
                hint: self.hint.clone(),
                duration: self.duration,
            };
            let _ = emit(ctx.app, SCRIPT_INPUT, &payload);

            let user_input = rx.await.map_err(|_| anyhow!("用户输入通道已关闭"))?;

            // ---- 添加用户输入台词先 ----
            let player_name = ctx.game_status.lock().await.player.user_name.clone();
            let line = crate::ai_service::types::LineBase {
                content: user_input.clone(),
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some(player_name),
                // 玩家台词一律标 sender_role_id=0（玩家），与 handle_user_message 对齐
                sender_role_id: Some(0),
                ..Default::default()
            };
            append_free_dialogue_line_if_current(&ctx.game_status, ctx.db, request_session, line)
                .await?;

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
                append_free_dialogue_line_if_current(
                    &ctx.game_status,
                    ctx.db,
                    request_session,
                    sys_line,
                )
                .await?;
            }

            // ---- 调用 AI 生成回复 ----
            generator.process_message(None).await?;
            ensure_history_session_current(&ctx.game_status, request_session).await?;

            if is_last_round {
                break;
            }
        }

        // The final generator await may have raced a Preview transition; do
        // not emit a completion marker for an event whose canonical session is
        // no longer current.
        ensure_history_session_current(&ctx.game_status, request_session).await?;

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
}

pub fn register() {
    register_event(FreeDialogueEvent::event_type(), |data| {
        Box::new(FreeDialogueEvent::from_event_data(&data))
    });
}

#[cfg(all(test, feature = "memory-test-api"))]
mod tests {
    use super::*;
    use crate::ai_service::game_system::role_manager::GameRoleManager;
    use crate::ai_service::memory::{MemoryConfig, MemorySectionLimits};
    use crate::config::tts::TtsConfig;
    use crate::memory_test_api::temp_db::TemporaryDatabase;
    use std::sync::Arc;

    fn status_for_test(db: &TemporaryDatabase) -> GameStatus {
        GameStatus::new(GameRoleManager::new(
            db.directory.path().to_path_buf(),
            Arc::new(tokio::sync::RwLock::new(None)),
            TtsConfig::default(),
            None,
            MemoryConfig {
                enabled: false,
                update_interval: 1,
                recent_window: 0,
                limits: MemorySectionLimits::default(),
            },
        ))
    }

    fn user_line(content: &str) -> LineBase {
        LineBase {
            content: content.into(),
            attribute: LineAttributeExt(LineAttribute::User),
            sender_role_id: Some(0),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn preview_round_trip_rejects_free_dialogue_setup_and_later_round_writes() {
        let db = TemporaryDatabase::open().await.unwrap();
        let game_status = Arc::new(tokio::sync::Mutex::new(status_for_test(&db)));
        let stale_session = game_status.lock().await.history_session();

        // This models Preview entering and restoring while get_role, LLM slot
        // setup, or a generator round is awaiting. The original free-dialogue
        // event must neither admit a generator/tool nor write its player or
        // plot lines into restored canonical history.
        {
            let mut status = game_status.lock().await;
            status.role_manager.set_memory_preview(true);
            status.preview_generation = status.preview_generation.wrapping_add(1);
            status.role_manager.set_memory_preview(false);
            status.preview_generation = status.preview_generation.wrapping_add(1);
        }
        assert!(
            ensure_history_session_current(&game_status, stale_session)
                .await
                .is_err()
        );
        assert!(
            GameStatus::admit_tool_execution_if_current(&game_status, stale_session)
                .await
                .is_none()
        );
        assert!(
            append_free_dialogue_line_if_current(
                &game_status,
                &db.connection,
                stale_session,
                user_line("stale player input"),
            )
            .await
            .is_err()
        );
        assert!(
            append_free_dialogue_line_if_current(
                &game_status,
                &db.connection,
                stale_session,
                user_line("stale plot prompt"),
            )
            .await
            .is_err()
        );
        assert!(game_status.lock().await.line_list.is_empty());

        // A fresh, coherent session retains ordinary generator/tool admission
        // and conditional canonical writes.
        let current_session = game_status.lock().await.history_session();
        assert!(
            ensure_history_session_current(&game_status, current_session)
                .await
                .is_ok()
        );
        assert!(
            GameStatus::admit_tool_execution_if_current(&game_status, current_session)
                .await
                .is_some()
        );
        append_free_dialogue_line_if_current(
            &game_status,
            &db.connection,
            current_session,
            user_line("current player input"),
        )
        .await
        .unwrap();
        assert_eq!(game_status.lock().await.line_list.len(), 1);
    }
}
