//! Dialogue event — sets current_character and emits character dialogue lines.
//!
//! 逐行把 `text` 交给 `consume_sentence` 复用完整管线（情绪解析 → 翻译/TTS → 响应构建 → 写入台词），
//! 与 Python 版 `DialogueEvent` 复用 `process_sentence` 对齐。解析失败 / 富化失败时回退纯文本，
//! 保证固定台词不因管线问题丢失、不中断剧本。

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use tauri::Manager;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::utils::script_function;
use crate::ai_service::message_system::events::emit;
use crate::ai_service::message_system::generator::{
    consume_sentence, ReplyOverrides, SentenceDeps,
};
use crate::ai_service::message_system::responses::ReplyResponse;
use crate::ai_service::types::{LineAttributeExt, LineBase};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::replace_placeholder;
use crate::AppState;

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
    ) -> Result<()> {
        // 试玩中该事件同样 emit ai:reply：带上当前试玩代号，前端据此丢弃
        // 试玩中止后迟到的固定台词（与 ai_dialogue 的 preview_gen 语义一致）
        let preview_gen = if ctx.is_preview {
            Some(ctx.game_status.lock().await.preview_generation)
        } else {
            None
        };
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
            preview_gen,
        };
        let _ = emit(ctx.app, "ai:reply", &payload);

        // Add ASSISTANT line
        let line = LineBase {
            content: text.to_string(),
            attribute: LineAttributeExt(LineAttribute::Assistant),
            sender_role_id: Some(role_id),
            display_name: Some(display_name.to_string()),
            original_emotion: Some(emotion.to_string()),
            ..Default::default()
        };
        ctx.game_status.lock().await.add_line(ctx.db, line).await?;
        Ok(())
    }
}

#[async_trait]
impl ScriptEvent for DialogueEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        let script_status = ctx
            .game_status
            .lock()
            .await
            .script_status
            .clone()
            .ok_or_else(|| anyhow!("ScriptStatus 未设置"))?;

        let (role_id, role_display_name) = {
            let mut gs = ctx.game_status.lock().await;
            let role = script_function::get_role(&mut *gs, ctx.db, &script_status, &self.character)
                .await?;
            let id = role.role_id.ok_or_else(|| anyhow!("角色 ID 未设置"))?;
            let dn = role.display_name.clone();
            (id, dn)
        };

        // 设为当前角色：consume_sentence 据此读取角色信息与 TTS 配置
        ctx.game_status.lock().await.current_role_id = Some(role_id);

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
                // 捕获当前试玩代号：中止后游离写入会被 add_assistant_line 的守卫丢弃
                generation: ctx.game_status.lock().await.preview_generation,
                is_preview: ctx.is_preview,
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
                }
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
                    )
                    .await?;
                }
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
                    )
                    .await?;
                }
            }
        }

        Ok(None)
    }

    fn event_type() -> &'static str {
        "dialogue"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(DialogueEvent::event_type(), |data| {
        Box::new(DialogueEvent::from_event_data(&data))
    });
}
