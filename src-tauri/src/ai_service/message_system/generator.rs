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
use crate::ai_service::god_agent::GodAgentCore;
use crate::ai_service::message_system::processor::{EmotionSegment, MessageProcessor, UserMessageOutcome};
use crate::ai_service::message_system::producer::{SentenceItem, StreamProducer};
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::responses::{event_names, ReplyResponse};
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
    /// 上帝 Agent（多人自由对话编排器），`None` 时退化为单角色对话。
    pub god_agent: Option<Arc<GodAgentCore>>,
}

/// `process_message` 各步骤间传递的用户消息上下文。
struct UserMessageContext {
    /// 处理后的完整消息（含 temp 段）。
    processed: String,
    /// 临时消息段（如有）。
    temp: Option<String>,
    /// 插入的用户行在 line_list 中的索引。
    line_index: Option<usize>,
    /// 用户消息序号（1-indexed，按 sender_role_id==0 且 User 属性计数）。
    seq: Option<u32>,
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
    ///
    /// 在多人自由对话模式下（God Agent 激活），会自动循环生成多轮 NPC 对话。
    pub async fn process_message(&self, user_message: Option<String>) -> Result<String> {
        // 1. 处理用户消息
        let user_ctx = self.handle_user_message(user_message.as_deref()).await?;

        // 1.5. 场景变化检测
        self.detect_scene_change().await?;

        // 2. 上帝 Agent 预处理：用户发消息时，先决定谁回应
        if user_message.is_some() {
            self.god_agent_pre_select().await?;
        }

        // 3. 生成循环（God Agent 激活时可能多轮）
        let mut accumulated = String::new();
        let mut consecutive_npc_rounds: usize = 0;
        let original_msg = user_message.unwrap_or_default();

        loop {
            // 取当前角色记忆（每轮重新获取，因为 current_role_id 可能已变化）
            let context = self.get_current_context().await?;
            if context.is_empty() {
                break;
            }

            // 启动 LLM 流生成
            let round_msg_seq = if consecutive_npc_rounds == 0 {
                user_ctx.seq
            } else {
                None
            };
            let round_acc = self
                .execute_pipeline(context, &original_msg, round_msg_seq)
                .await?;
            accumulated.push_str(&round_acc);

            // 后处理：仅第一轮清理 temp_message
            if consecutive_npc_rounds == 0 {
                self.cleanup_temp_message(&user_ctx).await?;
            }

            consecutive_npc_rounds += 1;

            // 上帝 Agent 后处理：决定下一个说话者
            let (should_continue, _next_role) =
                self.god_agent_post_select(consecutive_npc_rounds).await?;
            if !should_continue {
                break;
            }
        }

        Ok(accumulated)
    }

    // ============================================================
    // 子步骤
    // ============================================================

    /// Step 1: 预处理用户消息，构建 USER Line 并写入 GameStatus。
    ///
    /// 返回 `UserMessageContext` 供后续步骤使用。
    async fn handle_user_message(&self, raw: Option<&str>) -> Result<UserMessageContext> {
        let Some(raw) = raw else {
            return Ok(UserMessageContext {
                processed: String::new(),
                temp: None,
                line_index: None,
                seq: None,
            });
        };

        let UserMessageOutcome { main, temp } =
            self.deps.processor.append_user_message(raw).await;

        let mut gs = self.deps.game_status.lock().await;
        let user_name = gs.player.user_name.clone();
        let line = LineBase {
            content: main.clone(),
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(user_name),
            sender_role_id: Some(0),
            ..Default::default()
        };
        gs.add_line(&self.deps.db, line).await?;
        let line_index = Some(gs.line_list.len().saturating_sub(1));
        let seq = Some(
            gs.line_list
                .iter()
                .filter(|l| {
                    l.base.sender_role_id == Some(0)
                        && matches!(l.attribute(), LineAttribute::User)
                })
                .count() as u32,
        );

        Ok(UserMessageContext {
            processed: main,
            temp,
            line_index,
            seq,
        })
    }

    /// Step 1.5: 检测场景变化，若场景切换则添加系统旁白台词。
    async fn detect_scene_change(&self) -> Result<()> {
        let mut gs = self.deps.game_status.lock().await;
        if !gs.scene_awareness_enabled
            || gs.current_scene_id.is_none()
            || gs.current_scene_id == gs.last_processed_scene_id
        {
            return Ok(());
        }

        let scene_id = gs.current_scene_id.clone().unwrap();
        let store = SceneStore::new(&data_dir());
        if let Ok(Some(scene)) = store.find_by_id(&scene_id) {
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
        Ok(())
    }

    /// Step 2: 根据 current_role_id 获取当前角色的 memory 上下文。
    async fn get_current_context(&self) -> Result<Vec<LlmMessage>> {
        let mut gs = self.deps.game_status.lock().await;
        let Some(rid) = gs.current_role_id else {
            tracing::error!("生成消息的时候没有当前角色，取消生成");
            return Ok(Vec::new());
        };
        let role = gs.get_role(&self.deps.db, rid).await?;
        Ok(role.memory.clone())
    }

    /// Step 3: 启动 LLM 流管道，统一处理 thinking emit 与错误分发。
    async fn execute_pipeline(
        &self,
        context: Vec<LlmMessage>,
        user_message: &str,
        user_msg_seq: Option<u32>,
    ) -> Result<String> {
        events::emit_thinking(&self.deps.app, true);

        match self
            .run_pipeline(context, user_message.to_string(), user_msg_seq)
            .await
        {
            Ok(acc) => {
                events::emit_thinking(&self.deps.app, false);
                Ok(acc)
            }
            Err(e) => {
                events::emit_error(&self.deps.app, &e);
                events::emit_thinking(&self.deps.app, false);
                Err(e)
            }
        }
    }

    /// Step 4: 后处理 — 若存在 temp_message，将 user 行中的 temp 段清理后重建记忆。
    async fn cleanup_temp_message(&self, ctx: &UserMessageContext) -> Result<()> {
        let (Some(temp), Some(idx)) = (ctx.temp.as_deref(), ctx.line_index) else {
            return Ok(());
        };
        let mut gs = self.deps.game_status.lock().await;
        if let Some(line) = gs.line_list.get_mut(idx) {
            line.base.content = ctx.processed.replace(temp, "");
        }
        gs.refresh_memories(&self.deps.db).await?;
        Ok(())
    }

    // ============================================================
    // 上帝 Agent 集成
    // ============================================================

    /// 预处理：用户发消息时，上帝 Agent 决定哪个角色先回应。
    async fn god_agent_pre_select(&self) -> Result<()> {
        let Some(god) = &self.deps.god_agent else {
            return Ok(());
        };

        let (should_activate, current_speaker) = {
            let gs = self.deps.game_status.lock().await;
            (god.should_activate(&gs), gs.current_role_id)
        };
        if !should_activate {
            return Ok(());
        }

        // 决策下一个说话者
        let (selected_role_id, reason) = {
            let gs = self.deps.game_status.lock().await;
            god.decide_next_speaker(&gs, current_speaker).await?
        };

        if selected_role_id == 0 {
            return Ok(()); // 选择玩家，保持现状
        }

        // 设定新的 current_role_id
        let character_name = {
            let mut gs = self.deps.game_status.lock().await;
            gs.current_role_id = Some(selected_role_id);
            let role = gs.get_role(&self.deps.db, selected_role_id).await?;
            role.display_name.clone().unwrap_or_default()
        };

        tracing::info!(
            "[GodAgent] pre-select: role_id={}, name={}, reason={}",
            selected_role_id, character_name, reason
        );

        self.emit_character_switch(selected_role_id, &character_name);
        Ok(())
    }

    /// 后处理：消息生成完毕后，上帝 Agent 决定下一个说话者。
    ///
    /// 返回 `(should_continue, next_role_id)`：
    /// - `should_continue=true` 表示应继续循环（NPC 说话）
    /// - `should_continue=false` 表示应停止（交还玩家或 God Agent 未激活）
    async fn god_agent_post_select(
        &self,
        consecutive_npc_rounds: usize,
    ) -> Result<(bool, i32)> {
        let Some(god) = &self.deps.god_agent else {
            return Ok((false, 0));
        };

        // 检查是否超过连续 NPC 轮数上限
        if consecutive_npc_rounds >= god.config.max_consecutive_npc {
            tracing::info!(
                "[GodAgent] 连续 {} 轮 NPC 发言，强制返回玩家",
                consecutive_npc_rounds
            );
            return Ok((false, 0));
        }

        // 检查是否应激活
        let (should_activate, current_speaker) = {
            let gs = self.deps.game_status.lock().await;
            (god.should_activate(&gs), gs.current_role_id)
        };
        if !should_activate {
            return Ok((false, 0));
        }

        // 决策
        let (selected_role_id, reason) = {
            let gs = self.deps.game_status.lock().await;
            god.decide_next_speaker(&gs, current_speaker).await?
        };

        if selected_role_id == 0 {
            // 交还玩家
            return Ok((false, 0));
        }

        // 设定下一个说话者
        let character_name = {
            let mut gs = self.deps.game_status.lock().await;
            gs.current_role_id = Some(selected_role_id);
            let role = gs.get_role(&self.deps.db, selected_role_id).await?;
            role.display_name.clone().unwrap_or_default()
        };

        tracing::info!(
            "[GodAgent] post-select: role_id={}, name={}, reason={}",
            selected_role_id, character_name, reason
        );

        self.emit_character_switch(selected_role_id, &character_name);
        Ok((true, selected_role_id))
    }

    /// 通知前端当前说话角色已切换。
    fn emit_character_switch(&self, role_id: i32, name: &str) {
        let payload = serde_json::json!({
            "type": "character_switch",
            "roleId": role_id,
            "characterName": name,
        });
        if let Err(e) = self.deps.app.emit("character:switch", &payload) {
            tracing::warn!("emit character:switch 失败: {e}");
        }
    }

    async fn run_pipeline(
        &self,
        context: Vec<LlmMessage>,
        user_message: String,
        user_message_seq: Option<u32>,
    ) -> Result<String> {
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
                    let resp = match consume_sentence(
                        &deps,
                        cid,
                        sentence,
                        &user_message,
                        is_final,
                        user_message_seq,
                    )
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

}

// ============================================================
// consumer 句子处理
// ============================================================

/// 处理单个句子：解析 → 富化 → 构建响应 → 保存行。
async fn consume_sentence(
    deps: &GeneratorDeps,
    _consumer_id: usize,
    sentence: String,
    user_message: &str,
    is_final: bool,
    user_message_seq: Option<u32>,
) -> Result<Option<ReplyResponse>> {
    if sentence.is_empty() {
        return Ok(None);
    }

    // 1. 解析情绪分段
    let mut segments = parse_segments(deps, &sentence);
    if segments.is_empty() {
        return Ok(None);
    }

    // 2. 富化：翻译 + 语音
    enrich_segments(deps, &mut segments).await?;

    // 3. 构建前端响应
    let response =
        build_reply_response(deps, &segments, user_message, is_final, user_message_seq).await?;

    // 4. 写入 GameStatus
    add_assistant_line(deps, &response).await?;

    Ok(Some(response))
}

/// Step A: 解析并分类情绪片段。
fn parse_segments(deps: &GeneratorDeps, sentence: &str) -> Vec<EmotionSegment> {
    let segments = deps
        .processor
        .parse_and_classify_emotional_segments(sentence);
    if segments.is_empty() {
        tracing::warn!("AI 回复格式错误（未找到情绪 tag）");
    }
    segments
}

/// Step B: 翻译（中文→日文）与语音生成。
async fn enrich_segments(
    deps: &GeneratorDeps,
    segments: &mut [EmotionSegment],
) -> Result<()> {
    // 翻译：当第一段 japanese_text 为空时
    if segments[0].japanese_text.is_empty() {
        deps.translator
            .translate_segments(segments, false)
            .await?;
    }

    // 语音：取当前角色的 voice_maker 生成语音文件
    let voice_maker = {
        let gs = deps.game_status.lock().await;
        gs.current_role_id.and_then(|rid| {
            gs.role_manager
                .get_loaded(rid)
                .and_then(|r| r.voice_maker.clone())
        })
    };
    if let Some(vm) = voice_maker {
        vm.generate_voice_files(segments).await;
    }

    Ok(())
}

/// Step C: 构建 ReplyResponse（含角色信息填充）。
async fn build_reply_response(
    deps: &GeneratorDeps,
    segments: &[EmotionSegment],
    user_message: &str,
    is_final: bool,
    user_message_seq: Option<u32>,
) -> Result<ReplyResponse> {
    // 从 GameStatus 取当前角色信息
    let role_info: Option<(Option<String>, Option<i32>)> = {
        let gs = deps.game_status.lock().await;
        gs.current_role_id.and_then(|rid| {
            gs.role_manager
                .get_loaded(rid)
                .map(|role| (role.display_name.clone(), role.role_id))
        })
    };

    let first = &segments[0];
    let (character, role_id) = match role_info {
        Some((name, rid)) => (name, rid),
        None => (first.character.clone(), first.role_id),
    };

    let mut response = ReplyResponse::new_reply();
    response.character = character;
    response.role_id = role_id;
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
    response.user_message_seq = user_message_seq;

    Ok(response)
}

/// Step D: 将 assistant LINE 写入 GameStatus。
async fn add_assistant_line(deps: &GeneratorDeps, response: &ReplyResponse) -> Result<()> {
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
    let mut gs = deps.game_status.lock().await;
    gs.add_line(&deps.db, line).await?;
    Ok(())
}
