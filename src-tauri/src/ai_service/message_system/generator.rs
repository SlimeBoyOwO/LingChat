//! 消息生成协调器。对标 Python `MessageGenerator.process_message_stream`。
//!
//! 职责：
//! 1. 把用户消息（如有）走 MessageProcessor 预处理后，作为 USER 行入 GameStatus。
//! 2. 读取当前角色的 memory 作为 LLM 上下文。
//! 3. 启动 StreamProducer 从 LLM 流中切句子，送入 consumer 并行处理（情绪解析 + 翻译 + TTS）。
//! 4. 按顺序把 `ReplyResponse` 通过 Tauri `Emitter` 发给前端（event: `ai:reply`）。
//! 5. 每个段落作为 assistant LINE 入 GameStatus（带 TTS/动作/情绪）。

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result};
use sea_orm::DatabaseConnection;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::llm::LlmClient;
use crate::ai_service::message_system::processor::{MessageProcessor, UserMessageOutcome};
use crate::ai_service::message_system::producer::{SentenceItem, StreamProducer};
use crate::ai_service::message_system::responses::{
    event_names, ErrorResponse, ReplyResponse, StatusResetResponse, ThinkingResponse,
};
use crate::ai_service::translator::Translator;
use crate::ai_service::types::{LineAttributeExt, LineBase, LlmMessage};
use crate::api::data_dir;
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

/// MessageGenerator 运行时依赖。
#[derive(Clone)]
pub struct GeneratorDeps {
    pub app: AppHandle,
    pub db: DatabaseConnection,
    pub game_status: Arc<Mutex<GameStatus>>,
    pub processor: Arc<MessageProcessor>,
    pub translator: Arc<Translator>,
    pub llm: Arc<LlmClient>,
    pub concurrency: usize,
}

pub struct MessageGenerator {
    deps: GeneratorDeps,
}

impl MessageGenerator {
    pub fn new(deps: GeneratorDeps) -> Self {
        Self { deps }
    }

    /// 处理一轮用户消息。返回 accumulated LLM 原始输出（便于日志 / 单测）。
    ///
    /// 若 `user_message=None` 表示主动对话触发；此时会跳过 user 行构造，直接走
    /// `GameStatus` 的 current role memory 发起 LLM。
    pub async fn process_message(&self, user_message: Option<String>) -> Result<String> {
        // === 1. 处理用户消息 ===
        let mut processed_user_message = String::new();
        let mut temp_message: Option<String> = None;
        let mut inserted_user_line_index: Option<usize> = None;

        if let Some(raw) = user_message.as_deref() {
            let UserMessageOutcome { main, temp } =
                self.deps.processor.append_user_message(raw).await;
            processed_user_message = main.clone();
            temp_message = temp;

            let mut gs = self.deps.game_status.lock().await;
            let user_name = gs.player.user_name.clone();
            let line = LineBase {
                content: main,
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some(user_name),
                ..Default::default()
            };
            gs.add_line(&self.deps.db, line).await?;
            inserted_user_line_index = Some(gs.line_list.len().saturating_sub(1));
        }

        // === 1.5. 场景变化检测 ===
        {
            let mut gs = self.deps.game_status.lock().await;
            if gs.scene_awareness_enabled
                && gs.current_scene_id.is_some()
                && gs.current_scene_id != gs.last_processed_scene_id
            {
                let scene_id = gs.current_scene_id.clone().unwrap();
                let store = SceneStore::new(&data_dir());
                if let Ok(Some(scene)) = store.find_by_id(&scene_id) {
                    // 若场景无描述则跳过旁白台词
                    if !scene.description.trim().is_empty() {
                        let text = format!(
                            "你们一起去了新的场景 - \"{}\"，\"{}\"",
                            scene.name, scene.description
                        );
                        let prompt = PromptRole::Narrator.build_prompt(&text);
                        let line = LineBase {
                            content: prompt,
                            attribute: LineAttributeExt(LineAttribute::User),
                            display_name: Some("系统".to_string()),
                            ..Default::default()
                        };
                        let _ = gs.add_line(&self.deps.db, line).await;
                    }
                }
                gs.last_processed_scene_id = gs.current_scene_id.clone();
            }
        }

        // === 2. 取当前角色记忆 ===
        let current_context: Vec<LlmMessage> = {
            let mut gs = self.deps.game_status.lock().await;
            let Some(rid) = gs.current_role_id else {
                tracing::error!("生成消息的时候没有当前角色，取消生成");
                return Ok(String::new());
            };
            let role = gs.get_role(&self.deps.db, rid).await?;
            role.memory.clone()
        };

        // === 3. 启动 LLM 流 + 句子管道 ===
        self.emit_thinking(true);

        let accumulated = match self
            .run_pipeline(current_context, user_message.clone().unwrap_or_default())
            .await
        {
            Ok(v) => v,
            Err(e) => {
                self.emit_error(&e);
                self.emit_thinking(false);
                return Err(e);
            }
        };

        self.emit_thinking(false);

        // === 4. 后处理：若 temp_message 存在，需要把 user 行里的 temp 段清理掉后重建记忆 ===
        if let (Some(temp), Some(idx)) = (temp_message.as_deref(), inserted_user_line_index) {
            let mut gs = self.deps.game_status.lock().await;
            if let Some(line) = gs.line_list.get_mut(idx) {
                line.base.content = processed_user_message.replace(temp, "");
            }
            gs.refresh_memories(&self.deps.db).await?;
        }

        Ok(accumulated)
    }

    async fn run_pipeline(&self, context: Vec<LlmMessage>, user_message: String) -> Result<String> {
        let (sentence_tx, sentence_rx) =
            mpsc::channel::<SentenceItem>(self.deps.concurrency.max(1) * 2);
        let (publish_tx, mut publish_rx) =
            mpsc::channel::<(usize, Option<ReplyResponse>)>(self.deps.concurrency.max(1) * 2);

        // publisher：按索引顺序 emit 到前端
        let app = self.deps.app.clone();
        let publisher = tokio::spawn(async move {
            let mut next_index = 0usize;
            let mut buf: HashMap<usize, Option<ReplyResponse>> = HashMap::new();
            while let Some((idx, resp)) = publish_rx.recv().await {
                buf.insert(idx, resp);
                while let Some(item) = buf.remove(&next_index) {
                    next_index += 1;
                    if let Some(resp) = item {
                        let is_final = resp.is_final;
                        if let Err(e) = app.emit(event_names::AI_REPLY, &resp) {
                            tracing::warn!("emit ai:reply 失败: {e}");
                        }
                        if is_final {
                            return;
                        }
                    }
                }
            }
        });

        // consumer 池：并发处理句子
        let sentence_rx = Arc::new(Mutex::new(sentence_rx));
        let concurrency = self.deps.concurrency.max(1);
        let mut consumer_tasks = Vec::with_capacity(concurrency);
        for cid in 0..concurrency {
            let deps = self.deps.clone();
            let sentence_rx = sentence_rx.clone();
            let publish_tx = publish_tx.clone();
            let user_message = user_message.clone();
            consumer_tasks.push(tokio::spawn(async move {
                loop {
                    let item = {
                        let mut rx = sentence_rx.lock().await;
                        rx.recv().await
                    };
                    let Some((sentence, index, is_final)) = item else {
                        break;
                    };
                    let resp = match consume_sentence(&deps, cid, sentence, &user_message, is_final)
                        .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!("consumer {cid} 处理句子失败: {e}");
                            None
                        }
                    };
                    let _ = publish_tx.send((index, resp)).await;
                    if is_final {
                        break;
                    }
                }
            }));
        }
        drop(publish_tx);

        // producer：LLM 流 -> 句子
        let llm_stream = self.deps.llm.complete_stream(&context).await?;
        let producer = StreamProducer::new(llm_stream, sentence_tx);
        let acc = producer.run().await.context("StreamProducer 失败")?;

        for t in consumer_tasks {
            let _ = t.await;
        }
        let _ = publisher.await;

        Ok(acc)
    }

    fn emit_thinking(&self, is_thinking: bool) {
        let payload = ThinkingResponse::new(is_thinking);
        if let Err(e) = self.deps.app.emit(event_names::AI_THINKING, &payload) {
            tracing::warn!("emit thinking 失败: {e}");
        }
    }

    fn emit_error(&self, err: &anyhow::Error) {
        let msg = err.to_string();
        let code = classify_error(&msg);
        let err_payload = ErrorResponse::new(code, &msg);
        let _ = self.deps.app.emit(event_names::AI_ERROR, &err_payload);
        let reset = StatusResetResponse::new("input");
        let _ = self.deps.app.emit(event_names::STATUS_RESET, &reset);
    }
}

fn classify_error(msg: &str) -> &'static str {
    let lc = msg.to_lowercase();
    if msg.contains("401") || msg.contains("Api key is invalid") {
        "401"
    } else if msg.contains("404") {
        "404"
    } else if lc.contains("network") || msg.contains("网络") {
        "network_error"
    } else {
        "default_error"
    }
}

async fn consume_sentence(
    deps: &GeneratorDeps,
    consumer_id: usize,
    sentence: String,
    user_message: &str,
    is_final: bool,
) -> Result<Option<ReplyResponse>> {
    if sentence.is_empty() {
        return Ok(None);
    }

    let mut segments = deps
        .processor
        .parse_and_classify_emotional_segments(&sentence);
    if segments.is_empty() {
        tracing::warn!("AI 回复格式错误（未找到情绪 tag）");
        return Ok(None);
    }

    // 翻译（当第一段 japanese_text 为空时）
    if segments[0].japanese_text.is_empty() {
        deps.translator
            .translate_segments(&mut segments, false)
            .await?;
    }

    // VoiceMaker：取当前角色的 voice_maker（若配置）并生成语音文件
    let voice_maker = {
        let gs = deps.game_status.lock().await;
        gs.current_role_id.and_then(|rid| {
            gs.role_manager
                .get_loaded(rid)
                .and_then(|r| r.voice_maker.clone())
        })
    };
    if let Some(vm) = voice_maker {
        vm.generate_voice_files(&mut segments).await;
    }

    // 从 GameStatus 取当前角色信息，填入第一个段
    let role_info: Option<(Option<String>, Option<i32>)> = {
        let gs = deps.game_status.lock().await;
        gs.current_role_id.and_then(|rid| {
            gs.role_manager
                .get_loaded(rid)
                .map(|role| (role.display_name.clone(), role.role_id))
        })
    };
    if let Some((name, rid)) = role_info {
        let first = &mut segments[0];
        first.character = name;
        first.role_id = rid;
    }

    let first = &segments[0];
    let mut response = ReplyResponse::new_reply();
    response.character = first.character.clone();
    response.role_id = first.role_id;
    response.emotion = if !first.predicted.is_empty() {
        first.predicted.clone()
    } else {
        first.original_tag.clone()
    };
    response.original_tag = first.original_tag.clone();
    response.message = first.following_text.clone();
    response.tts_text = if first.japanese_text.is_empty() {
        None
    } else {
        Some(first.japanese_text.clone())
    };
    response.motion_text = if first.motion_text.is_empty() {
        None
    } else {
        Some(first.motion_text.clone())
    };
    response.audio_file = if first.voice_file.is_empty() {
        None
    } else {
        let p = std::path::Path::new(&first.voice_file);
        if p.exists() {
            p.file_name().map(|n| n.to_string_lossy().to_string())
        } else {
            None
        }
    };
    response.original_message = user_message.to_string();
    response.is_final = is_final;

    // assistant LINE 入 GameStatus
    let line = LineBase {
        content: response.message.clone(),
        sender_role_id: response.role_id,
        original_emotion: Some(response.original_tag.clone()),
        predicted_emotion: Some(response.emotion.clone()),
        tts_content: response.tts_text.clone(),
        action_content: response.motion_text.clone(),
        audio_file: response.audio_file.clone(),
        display_name: response.character.clone(),
        attribute: LineAttributeExt(LineAttribute::Assistant),
        ..Default::default()
    };
    {
        let mut gs = deps.game_status.lock().await;
        gs.add_line(&deps.db, line).await?;
    }

    Ok(Some(response))
}
