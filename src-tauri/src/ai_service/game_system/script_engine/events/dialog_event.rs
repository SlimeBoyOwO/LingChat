//! Dialogue event — sets current_character and emits character dialogue lines.
//!
//! 逐行把 `text` 交给 `consume_sentence` 复用完整管线（情绪解析 → 翻译/TTS → 响应构建 → 写入台词），
//! 与 Python 版 `DialogueEvent` 复用 `process_sentence` 对齐。解析失败 / 富化失败时回退纯文本，
//! 保证固定台词不因管线问题丢失、不中断剧本。

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tauri::Manager;

use crate::AppState;
use crate::ai_service::game_system::game_status::HistorySession;
use crate::ai_service::game_system::script_engine::events::{
    ScriptContext, ScriptEvent, parse_duration, register_event,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::generator::{
    ReplyOverrides, SentenceDeps, consume_sentence,
};
use crate::ai_service::message_system::responses::ReplyResponse;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::replace_placeholder;

pub struct DialogueEvent {
    character: String,
    text: String,
    display_name: Option<String>,
    display_subtitle: Option<String>,
    emotion: Option<String>,
    duration: Option<f64>,
}

impl DialogueEvent {
    fn from_event_data(data: &Value) -> Self {
        Self {
            character: data
                .get("character")
                .and_then(|v| v.as_str())
                .unwrap_or("MAIN")
                .to_string(),
            text: data
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            display_name: data
                .get("displayName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            display_subtitle: data
                .get("displaySubtitle")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            emotion: data
                .get("emotion")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            duration: parse_duration(data),
        }
    }

    /// 回退路径：把原始文本作为普通回复发出并写入台词。
    /// 供解析不到【情绪】分段 / 富化失败时兜底，保证固定台词不丢失。
    async fn emit_plain_reply(
        &self,
        ctx: &ScriptContext<'_>,
        role_id: i32,
        text: &str,
        display_name: &str,
        display_subtitle: &str,
        emotion: &str,
        session: HistorySession,
    ) -> Result<()> {
        // Write before emitting, using the same captured identity as sentence
        // enrichment. A fallback must not re-capture mode/generation after its
        // await and publish a reply for restored canonical history.
        let line = LineBase {
            content: text.to_string(),
            attribute: LineAttributeExt(LineAttribute::Assistant),
            sender_role_id: Some(role_id),
            display_name: Some(display_name.to_string()),
            original_emotion: Some(emotion.to_string()),
            ..Default::default()
        };
        if !ctx
            .game_status
            .lock()
            .await
            .append_line_if_current(ctx.db, session, line)
            .await?
        {
            return Err(anyhow!("固定台词期间历史会话已切换，已丢弃本次事件"));
        }

        let payload = ReplyResponse {
            type_: "reply".to_string(),
            duration: self.duration.unwrap_or(-1.0),
            is_final: true,
            character: Some(self.character.clone()),
            role_id: Some(role_id),
            emotion: emotion.to_string(),
            original_tag: String::new(),
            message: text.to_string(),
            tts_text: None,
            motion_text: None,
            audio_file: None,
            original_message: text.to_string(),
            display_name: Some(display_name.to_string()),
            display_subtitle: Some(display_subtitle.to_string()),
            user_message_seq: None,
            thinking: None,
            preview_gen: session.is_preview.then_some(session.generation),
        };
        let _ = emit(ctx.app, "ai:reply", &payload);
        Ok(())
    }
}

#[async_trait]
impl ScriptEvent for DialogueEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let (script_status, request_session) = {
            let gs = ctx.game_status.lock().await;
            (
                gs.script_status
                    .clone()
                    .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?,
                gs.history_session(),
            )
        };

        let (role_id, role_display_name) = {
            let mut gs = ctx.game_status.lock().await;
            let role = script_function::get_role(&mut *gs, ctx.db, &script_status, &self.character)
                .await?;
            let id = role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?;
            let dn = role.display_name.clone();
            (id, dn)
        };

        // 设为当前角色：consume_sentence 据此读取角色信息与 TTS 配置。
        // Role lookup may await, so the original session must still be current.
        {
            let mut gs = ctx.game_status.lock().await;
            if !gs.is_history_session_current(request_session) {
                return Err(anyhow!("固定台词期间历史会话已切换，已丢弃本次事件"));
            }
            gs.current_role_id = Some(role_id);
        }

        // Get display info
        let display_name = self
            .display_name
            .clone()
            .or(role_display_name)
            .unwrap_or_default();
        let display_subtitle = self.display_subtitle.clone().unwrap_or_default();
        let emotion = self.emotion.clone().unwrap_or_default();

        // 构建句子处理依赖。consume_sentence 不依赖 LLM，未配置模型也能跑固定台词。
        let sdeps = {
            let state = ctx.app.state::<AppState>();
            SentenceDeps {
                processor: state.chat.processor.clone(),
                translator: state.chat.translator.clone(),
                game_status: ctx.game_status.clone(),
                db: ctx.db.clone(),
                session: request_session,
                stale: Arc::new(AtomicBool::new(false)),
            }
        };
        let overrides = ReplyOverrides {
            display_name: Some(display_name.clone()),
            display_subtitle: Some(display_subtitle.clone()),
            duration: self.duration,
        };
        let thinking_buf = tokio::sync::Mutex::new(String::new());

        // 替换占位符（%player%），与 Python 版对齐
        let text = {
            let gs = ctx.game_status.lock().await;
            replace_placeholder(&self.text, &gs)
        };

        // 逐行复用 consume_sentence：解析 → 富化（翻译/TTS）→ 构建响应 → 写入台词
        for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
            match consume_sentence(
                &sdeps,
                line.to_string(),
                "",
                true,
                None,
                &thinking_buf,
                &overrides,
            )
            .await
            {
                Ok(Some(resp)) => {
                    let _ = emit(ctx.app, "ai:reply", &resp);
                },
                Ok(None) => {
                    // 解析不到【情绪】分段：回退纯文本，保留已有纯文本剧本
                    tracing::warn!("dialogue 文本未解析到【情绪】分段，回退纯文本: {line}");
                    self.emit_plain_reply(
                        ctx,
                        role_id,
                        line,
                        &display_name,
                        &display_subtitle,
                        &emotion,
                        request_session,
                    )
                    .await?;
                },
                Err(e) => {
                    // 富化（翻译/TTS）失败不应中断剧本，回退纯文本
                    tracing::error!("consume_sentence 处理 dialogue 失败，回退纯文本: {e}");
                    self.emit_plain_reply(
                        ctx,
                        role_id,
                        line,
                        &display_name,
                        &display_subtitle,
                        &emotion,
                        request_session,
                    )
                    .await?;
                },
            }
        }

        Ok(None)
    }

    fn event_type() -> &'static str {
        "dialogue"
    }
}

pub fn register() {
    register_event(DialogueEvent::event_type(), |data| {
        Box::new(DialogueEvent::from_event_data(&data))
    });
}
