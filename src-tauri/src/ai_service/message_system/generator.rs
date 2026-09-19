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
use tokio::sync::{Mutex, mpsc, oneshot};

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::game_system::scene_store::SceneStore;
use crate::ai_service::god_agent::GodAgentCore;
use crate::ai_service::llm::LlmClient;
use crate::ai_service::message_system::events;
use crate::ai_service::message_system::processor::{
    EmotionSegment, MessageProcessor, UserMessageOutcome,
};
use crate::ai_service::message_system::producer::{SentenceItem, StreamProducer};
use crate::ai_service::message_system::responses::{ReplyResponse, event_names};
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::tools::tool_loop::stream_with_tool_loop;
use crate::ai_service::translator::Translator;
use crate::ai_service::tts::VoiceMaker;
use crate::ai_service::types::{GameLine, LineAttributeExt, LineBase, LlmMessage};
use crate::api::data_dir;
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

/// MessageGenerator 的业务调用来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratorSource {
    UserChat,
    Proactive,
    ScriptAiDialogue,
    ScriptFreeDialogue,
    EntryGreeting,
}

/// MessageGenerator 运行时依赖。
#[derive(Clone)]
pub struct GeneratorDeps {
    /// 调用本轮生成的业务来源。
    pub source: GeneratorSource,
    pub app: AppHandle,
    pub db: DatabaseConnection,
    pub game_status: Arc<Mutex<GameStatus>>,
    pub processor: Arc<MessageProcessor>,
    pub translator: Arc<Translator>,
    /// 当前生成轮次使用的 LLM 客户端快照（构建 deps 时从槽位读取）。
    pub llm: Arc<LlmClient>,
    /// 普通聊天可调用的共享工具注册表。
    pub tool_registry: Arc<ToolRegistry>,
    pub concurrency: usize,
    /// 上帝 Agent（多人自由对话编排器），`None` 时退化为单角色对话。
    pub god_agent: Option<Arc<GodAgentCore>>,
    /// 抑制 ai:thinking 事件。用于系统触发的后台生成（如入场问候）。
    pub suppress_thinking: bool,
    /// 构建 deps 时捕获的 `GameStatus.preview_generation`。写入台词前比对，
    /// 不一致说明本轮生成已过期（试玩被中止后游离任务仍在写），丢弃写入。
    /// 自由对话的代号恒为当前值，比对恒等，行为不变。
    pub generation: u64,
    /// 是否运行在编辑器试玩中。为 true 时回复带 `preview_gen` 标记，
    /// 前端据此丢弃中止后迟到的流式回复。
    pub is_preview: bool,
    /// 当轮附带的多模态图片（`data:image/...;base64,...` data URL）。
    /// 由「该模型支持原生识图」的对话路径设置：图片仅拼接进**当轮** LLM 上下文，
    /// 不写入角色记忆，从源头控制上下文/缓存占用。
    pub transient_image: Option<String>,
}

/// `process_message` 各步骤间传递的用户消息上下文。
struct UserMessageContext {
    /// 处理后的完整消息（含 temp 段）。
    processed: String,
    /// 临时消息段（如有）。
    temp: Option<String>,
    /// 插入的用户行在 line_list 中的索引。
    line_index: Option<usize>,
    /// 用户消息序号（1-indexed，按「玩家身份实体 sender」且 User 属性计数）。
    /// 附身 AI 时本轮输入不属于玩家身份消息，为 None。
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
    /// `None` 只表示本轮没有原始用户输入；业务调用来源由 `GeneratorDeps::source` 表示。
    /// 此时会跳过 user 行构造，直接走 `GameStatus` 的 current role memory 发起 LLM。
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
            // 每轮记录生成判据：附身实体应休眠跳过，便于排查"无人回应"
            {
                let gs = self.deps.game_status.lock().await;
                tracing::info!(
                    "生成轮次开始: current_role_id={:?}, possessed={}, 休眠跳过={}",
                    gs.current_role_id,
                    gs.possessed_role_id,
                    gs.current_role_id.is_some_and(|rid| gs.is_possessed(rid)),
                );
            }
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

    /// 把后台工具完成通知作为仅对 LLM 可见的临时 user context 触发新一轮回复。
    /// 通知不会写成玩家台词，避免界面和历史中出现伪造的用户消息。
    #[cfg_attr(not(desktop), allow(dead_code))]
    pub async fn process_notification(&self, notification: String) -> Result<String> {
        let mut context = self.get_current_context().await?;
        if context.is_empty() {
            return Ok(String::new());
        }
        context.push(LlmMessage::user(notification));
        self.execute_pipeline(context, "", None).await
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

        let UserMessageOutcome { main, temp } = self.deps.processor.append_user_message(raw).await;

        let mut gs = self.deps.game_status.lock().await;
        let user_name = gs.player.user_name.clone();
        let sender_role_id = gs.possessed_role_id;
        let line = LineBase {
            content: main.clone(),
            attribute: LineAttributeExt(LineAttribute::User),
            display_name: Some(user_name),
            // 玩家台词的归属 = 当前被附身实体；默认身份仍是 PLAYER_ROLE_ID(0)
            sender_role_id: Some(sender_role_id),
            ..Default::default()
        };
        gs.add_line(&self.deps.db, line).await?;
        let line_index = Some(gs.line_list.len().saturating_sub(1));
        // 回溯序号按 `attribute == User` 且有明确发送者判定，与
        // compute_user_message_seqs 同源：附身期间玩家发言的 sender 是被附身实体，
        // 不能再按玩家身份集合过滤；sender 为空的系统旁白不计入。
        let seq = Some(
            gs.line_list
                .iter()
                .filter(|l| {
                    l.base.sender_role_id.is_some()
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
    ///
    /// 被附身实体的生成休眠挂在这里：玩家正以它发言，AI 不能再替它说话。
    /// 返回空上下文后，主循环与 `process_notification` 的 `is_empty()` 分支天然
    /// 跳过本轮生成，无需改动主循环。
    async fn get_current_context(&self) -> Result<Vec<LlmMessage>> {
        let mut gs = self.deps.game_status.lock().await;
        let Some(rid) = gs.current_role_id else {
            tracing::error!("生成消息的时候没有当前角色，取消生成");
            return Ok(Vec::new());
        };
        if gs.is_possessed(rid) {
            tracing::info!("当前角色 {} 正被玩家附身，AI 生成休眠", rid);
            return Ok(Vec::new());
        }
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
        if !self.deps.suppress_thinking {
            events::emit_thinking(&self.deps.app, true);
        }

        match self
            .run_pipeline(context, user_message.to_string(), user_msg_seq)
            .await
        {
            Ok(acc) => {
                if !self.deps.suppress_thinking {
                    events::emit_thinking(&self.deps.app, false);
                }
                Ok(acc)
            },
            Err(e) => {
                events::emit_error(&self.deps.app, &e);
                if !self.deps.suppress_thinking {
                    events::emit_thinking(&self.deps.app, false);
                }
                Err(e)
            },
        }
    }

    /// Step 4: 后处理 — 若存在 temp_message，将 user 行中的 temp 段清理后重建记忆。
    async fn cleanup_temp_message(&self, ctx: &UserMessageContext) -> Result<()> {
        let (Some(temp), Some(idx)) = (ctx.temp.as_deref(), ctx.line_index) else {
            return Ok(());
        };
        let mut gs = self.deps.game_status.lock().await;
        gs.role_manager.invalidate_memory_history();
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

        // 选中当前被附身实体 = 上帝把话筒交还玩家，保持现状
        let return_to_player = {
            let gs = self.deps.game_status.lock().await;
            gs.is_possessed(selected_role_id)
        };
        if return_to_player {
            return Ok(());
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
            selected_role_id,
            character_name,
            reason
        );

        self.emit_character_switch(selected_role_id, &character_name);
        Ok(())
    }

    /// 后处理：消息生成完毕后，上帝 Agent 决定下一个说话者。
    ///
    /// 返回 `(should_continue, next_role_id)`：
    /// - `should_continue=true` 表示应继续循环（NPC 说话）
    /// - `should_continue=false` 表示应停止（交还玩家或 God Agent 未激活）
    async fn god_agent_post_select(&self, consecutive_npc_rounds: usize) -> Result<(bool, i32)> {
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

        // 选中当前被附身实体 = 交还玩家
        let (return_to_player, possessed) = {
            let gs = self.deps.game_status.lock().await;
            (gs.is_possessed(selected_role_id), gs.possessed_role_id)
        };
        if return_to_player {
            return Ok((false, possessed));
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
            selected_role_id,
            character_name,
            reason
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
        // 本轮角色快照：在 god 选人/写 current_role_id 之后、流水线启动之前一次性捕获。
        // 其后整轮（prompt 角色名、翻译/TTS 音色、回复归属）都只读本快照，不再各自去读
        // 可变的 gs.current_role_id，避免长 await 期间角色漂移导致音色与显示角色错配。
        let snapshot = {
            let mut gs = self.deps.game_status.lock().await;
            let Some(role_id) = gs.current_role_id else {
                return Err(anyhow::anyhow!("工具调用时没有当前角色"));
            };
            // 惰性注册：保证显示名与音色字段取自同一份已加载角色
            let display_name = gs
                .get_role(&self.deps.db, role_id)
                .await?
                .display_name
                .clone();
            let loaded = gs.role_manager.get_loaded(role_id);
            RoundRoleSnapshot {
                role_id: Some(role_id),
                display_name,
                voice_maker: loaded.and_then(|role| role.voice_maker.clone()),
                tts_type: loaded
                    .and_then(|role| role.settings.tts_type.clone())
                    .unwrap_or_default(),
                voice_lang: loaded
                    .and_then(|role| role.settings.voice_lang.clone())
                    .unwrap_or_default(),
            }
        };
        let role_name = snapshot.display_name.clone();
        // 原生多模态识图：把当轮图片作为一条独立的用户消息拼进 LLM 上下文，
        // 仅本次请求可见，不回写记忆。放在末尾（紧跟最新用户输入之后的视觉提示），
        // 让模型把图片与最近的用户语境关联起来。
        let context = if let Some(image) = self.deps.transient_image.clone() {
            let mut ctx = context;
            let gs_guard = self.deps.game_status.lock().await;
            let user_name = gs_guard.player.user_name.clone();
            drop(gs_guard);
            let marker = if user_message.trim().is_empty() {
                format!("（用户「{}」发来一张图片，请查看图片内容。）", user_name)
            } else {
                format!(
                    "【图片】用户「{}」发来一张图片，请结合图片内容回复。",
                    user_name
                )
            };
            ctx.push(LlmMessage::user_with_image(marker, image));
            ctx
        } else {
            context
        };
        let tool_loop_result = stream_with_tool_loop(
            &self.deps.llm,
            &self.deps.tool_registry,
            context,
            self.deps.source,
            role_name,
            &self.deps.app,
        )
        .await?;
        // 惰性工具闭环：工具消息在流消费过程中才逐渐收集完整。
        // 先记下回填位置（当前台词末尾，即本轮助手回复写入之前），
        // 待流消费完毕后统一插入，保持「用户 → 工具消息 → 助手回复」的顺序。
        let tool_insert_pos = {
            let gs = self.deps.game_status.lock().await;
            gs.line_list.len()
        };
        let tool_messages = tool_loop_result.tool_messages;
        let tool_calls_seen = tool_loop_result.tool_calls_seen;
        let llm_stream = tool_loop_result.stream;

        let (sentence_tx, sentence_rx) =
            mpsc::channel::<SentenceItem>(self.deps.concurrency.max(1) * 2);
        let (publish_tx, publish_rx) =
            mpsc::channel::<PublishItem>(self.deps.concurrency.max(1) * 2);

        // producer 与 consumer 共享的思考链缓冲：累积本轮生成的完整思考文本，
        // 由最终句（is_final）的 consumer 快照并挂载到台词行与前端响应。
        let thinking_buf = Arc::new(Mutex::new(String::new()));

        // publisher：按索引顺序 emit 到前端（并在推进到栅栏索引时回 ack）
        let app = self.deps.app.clone();
        let publisher = tokio::spawn(async move {
            publish_ordered(publish_rx, |resp| {
                app.emit(event_names::AI_REPLY, resp)
                    .map_err(anyhow::Error::from)
            })
            .await
        });

        // consumer 池：并发处理句子
        let sentence_rx = Arc::new(Mutex::new(sentence_rx));
        let concurrency = self.deps.concurrency.max(1);
        let mut consumer_tasks = Vec::with_capacity(concurrency);
        for cid in 0..concurrency {
            let deps = self.deps.clone();
            // 每个 consumer 持有本轮快照的副本：角色归属与音色不随并发切换漂移
            let snapshot = snapshot.clone();
            let sentence_rx = sentence_rx.clone();
            let publish_tx = publish_tx.clone();
            let user_message = user_message.clone();
            let thinking_buf = thinking_buf.clone();
            consumer_tasks.push(tokio::spawn(async move {
                // 句子处理仅需最小依赖集；llm / 工具等不在消费端使用。
                let sdeps = SentenceDeps::from(&deps);
                loop {
                    let item = {
                        let mut rx = sentence_rx.lock().await;
                        rx.recv().await
                    };
                    let Some(item) = item else {
                        break;
                    };
                    let (sentence, index, is_final) = match item {
                        // 呈现栅栏不是句子：不解析、不翻译、不合成语音，只参与保序。
                        // ack 由 publisher 推进到该索引时回传；若 publisher 已退出则
                        // ack 随 send 失败被丢弃，工具侧据此 fail-closed。
                        SentenceItem::BeforeTools { index, ack } => {
                            if publish_tx
                                .send(PublishItem::BeforeTools { index, ack })
                                .await
                                .is_err()
                            {
                                break;
                            }
                            continue;
                        },
                        SentenceItem::Reply(sentence, index, is_final) => {
                            (sentence, index, is_final)
                        },
                    };
                    let resp = match consume_sentence_with_snapshot(
                        &sdeps,
                        &snapshot,
                        sentence,
                        &user_message,
                        is_final,
                        user_message_seq,
                        &thinking_buf,
                        &ReplyOverrides::default(),
                    )
                    .await
                    {
                        Ok(r) => r,
                        Err(e) => {
                            tracing::error!("consumer {cid} 处理句子失败: {e}");
                            None
                        },
                    };
                    let _ = publish_tx
                        .send(PublishItem::Reply {
                            index,
                            response: resp,
                        })
                        .await;
                    if is_final {
                        break;
                    }
                }
            }));
        }
        drop(publish_tx);

        // producer：LLM 流 -> 句子
        let producer = StreamProducer::new(
            llm_stream,
            sentence_tx,
            self.deps.app.clone(),
            thinking_buf,
            tool_calls_seen,
        );
        let output = producer.run().await.context("StreamProducer 失败")?;
        let acc = output.accumulated;

        for t in consumer_tasks {
            let _ = t.await;
        }
        // publisher 的返回值 = 是否成功发出过收尾句（`is_final`）。
        let published_final = publisher.await.unwrap_or(false);

        // 流已消费完毕，工具消息收集完整：回填到助手回复之前的位置
        let tool_msgs = std::mem::take(&mut *tool_messages.lock().await);
        if !tool_msgs.is_empty() {
            let mut gs = self.deps.game_status.lock().await;
            // 试玩代号守卫：试玩中止后丢弃迟到回填，与 add_assistant_line 行为一致
            if gs.preview_generation == self.deps.generation {
                gs.role_manager.invalidate_memory_history();
                let insert_pos = tool_insert_pos.min(gs.line_list.len());
                let perceived: Vec<i32> = gs.present_role_ids.iter().copied().collect();

                for msg in tool_msgs.iter().rev() {
                    let (attribute, content, tool_call) = match msg.role.as_str() {
                        "assistant" => {
                            let tool_call = msg
                                .tool_calls
                                .as_ref()
                                .map(|calls| serde_json::to_string(calls).unwrap_or_default());
                            (LineAttribute::Assistant, msg.content.clone(), tool_call)
                        },
                        "tool" => (
                            LineAttribute::Tool,
                            serde_json::to_string(&serde_json::json!({
                                "tool_call_id": msg.tool_call_id,
                                "result": serde_json::from_str::<serde_json::Value>(&msg.content)
                                    .unwrap_or(serde_json::Value::String(msg.content.clone())),
                            }))
                            .unwrap_or_default(),
                            None,
                        ),
                        _ => continue,
                    };
                    let line = LineBase {
                        content,
                        tool_call,
                        attribute: LineAttributeExt(attribute),
                        sender_role_id: None,
                        display_name: None,
                        ..Default::default()
                    };
                    gs.line_list
                        .insert(insert_pos, GameLine::from_base(line, perceived.clone()));
                }
                gs.refresh_memories(&self.deps.db).await?;
            }
        }

        // 空回复兜底：模型流没有任何正文时，主动通知前端并重置状态，
        // 否则界面会一直停在「思考中」
        if acc.trim().is_empty() {
            tracing::warn!("LLM 流未产生任何正文内容，重置前端状态");
            events::emit_error(
                &self.deps.app,
                &anyhow::anyhow!("模型没有返回任何内容，请再试一次"),
            );
        } else if !published_final {
            // 有正文、但没有一条 `is_final` 的回复成功落地——前端队列只认 is_final 或
            // status:reset 来复位，缺了它界面会永远停在等待态。工具闭环下这条是可达的：
            // 呈现栅栏已经把前导缓冲清空，工具后模型若不再产出正文，EOF 就没有可提升为
            // 终句的内容（旧行为会重放前导，但那会让用户看到同一句话两次）。
            // 这里只做温和复位：不重放、不弹错误提示。
            tracing::warn!(
                sent_final = output.sent_final,
                "本轮没有成功发布收尾句，仅复位前端状态"
            );
            events::emit_status_reset(&self.deps.app);
        }

        Ok(acc)
    }
}

// ============================================================
// 有序发布
// ============================================================

/// 投递到 publisher 的工作项。
///
/// 栅栏与回复共用同一个索引空间：publisher 只有在推进到栅栏索引（即所有更小索引
/// 都已处理完，包括被丢弃的 `None` 结果）之后才会回 ack，从而保证"前导台词的
/// `ai:reply` 已 emit"严格先于"工具事件 emit"。
pub(super) enum PublishItem {
    Reply {
        index: usize,
        response: Option<ReplyResponse>,
    },
    BeforeTools {
        index: usize,
        ack: oneshot::Sender<bool>,
    },
}

impl PublishItem {
    fn index(&self) -> usize {
        match self {
            Self::Reply { index, .. } | Self::BeforeTools { index, .. } => *index,
        }
    }
}

/// 按索引顺序发布；栅栏在其索引被推进到时回传"此前是否已发布过回复"。
///
/// 返回值 = **是否成功发出过收尾句（`is_final`）**：
/// - `true`：某条 `is_final` 的回复已 emit；
/// - `false`：`emit` 失败而中断，或通道关闭时都没等到终句。
///
/// `emit` 失败会立即返回，`pending` 里尚未回传的 ack 随函数一起被丢弃，
/// 工具侧因此 fail-closed（不会在回复发布失败后继续执行工具）。
///
/// 生产与顺序契约测试共用本函数（`ordering_tests`）。
pub(super) async fn publish_ordered(
    mut rx: mpsc::Receiver<PublishItem>,
    mut emit: impl FnMut(&ReplyResponse) -> Result<()>,
) -> bool {
    let mut next_index = 0usize;
    let mut reply_before_fence = false;
    let mut pending: HashMap<usize, PublishItem> = HashMap::new();
    while let Some(item) = rx.recv().await {
        pending.insert(item.index(), item);
        while let Some(item) = pending.remove(&next_index) {
            next_index += 1;
            match item {
                PublishItem::Reply {
                    response: Some(resp),
                    ..
                } => {
                    let is_final = resp.is_final;
                    if let Err(e) = emit(&resp) {
                        tracing::warn!("emit ai:reply 失败: {e}");
                        return false;
                    }
                    reply_before_fence = true;
                    if is_final {
                        return true;
                    }
                },
                PublishItem::Reply { response: None, .. } => {},
                PublishItem::BeforeTools { ack, .. } => {
                    let _ = ack.send(reply_before_fence);
                    reply_before_fence = false;
                },
            }
        }
    }
    false
}

// ============================================================
// consumer 句子处理
// ============================================================

/// 一轮生成的角色快照。
///
/// 一轮台词从 LLM 流生成到翻译、TTS 合成之间隔着多个长 await，期间
/// `GameStatus.current_role_id` 可能被工具调用、附身切换等不持生成锁的写路径改写。
/// 若让「prompt 角色名」「音色字段」「回复归属」各自去读这个可变值，同一轮内就会读到
/// 不同角色，导致音色与显示角色错配。故在每轮生成开始（角色已选定、流水线启动前）
/// 一次性捕获本快照，本轮全部环节只读快照。
#[derive(Clone, Default)]
struct RoundRoleSnapshot {
    /// 本轮归属角色 id；`None` 表示捕获时没有当前角色（剧本固定台词的兜底路径可能出现）。
    role_id: Option<i32>,
    /// 本轮角色显示名，取自已加载角色；缺省时由响应回退到情绪分段自带的角色。
    display_name: Option<String>,
    /// 本轮角色音色生成器，与显示名同源捕获，避免中途换人后音色错配。
    voice_maker: Option<VoiceMaker>,
    /// 本轮角色 TTS 引擎类型，决定翻译目标语言。
    tts_type: String,
    /// 本轮角色语音语言，决定翻译目标语言。
    voice_lang: String,
}

impl RoundRoleSnapshot {
    /// 从 `GameStatus` 的当前角色就地捕获快照（仅取已加载信息，不触发惰性加载）。
    ///
    /// 供没有跨轮快照的外部调用方（如剧本固定台词）使用：把「一句台词」当作一轮，
    /// 保证同一句内音色与归属也只读同一次结果。
    async fn capture(deps: &SentenceDeps) -> Self {
        let gs = deps.game_status.lock().await;
        let Some(role_id) = gs.current_role_id else {
            return Self::default();
        };
        let loaded = gs.role_manager.get_loaded(role_id);
        Self {
            role_id: Some(role_id),
            display_name: loaded.and_then(|role| role.display_name.clone()),
            voice_maker: loaded.and_then(|role| role.voice_maker.clone()),
            tts_type: loaded
                .and_then(|role| role.settings.tts_type.clone())
                .unwrap_or_default(),
            voice_lang: loaded
                .and_then(|role| role.settings.voice_lang.clone())
                .unwrap_or_default(),
        }
    }
}

/// `consume_sentence` 的最小依赖集。仅含句子处理真正用到的字段，
/// 不要求 LLM / 工具，剧本 `dialogue` 事件可在未配置模型时直接构建。
#[derive(Clone)]
pub struct SentenceDeps {
    pub processor: Arc<MessageProcessor>,
    pub translator: Arc<Translator>,
    pub game_status: Arc<Mutex<GameStatus>>,
    pub db: DatabaseConnection,
    /// 试玩代号（写入守卫用）。非试玩时传入当前值即可，守卫恒等。
    pub generation: u64,
    pub is_preview: bool,
}

impl From<&GeneratorDeps> for SentenceDeps {
    fn from(d: &GeneratorDeps) -> Self {
        Self {
            processor: d.processor.clone(),
            translator: d.translator.clone(),
            game_status: d.game_status.clone(),
            db: d.db.clone(),
            generation: d.generation,
            is_preview: d.is_preview,
        }
    }
}

/// 剧本固定台词（dialogue 事件）对 `consume_sentence` 构建响应的覆盖字段。
/// 生成路径用默认值，不覆盖任何字段。
#[derive(Default, Clone)]
pub struct ReplyOverrides {
    pub display_name: Option<String>,
    pub display_subtitle: Option<String>,
    pub duration: Option<f64>,
}

/// 处理单个句子：解析 → 富化 → 构建响应 → 保存行。
///
/// 供 MessageGenerator 的 consumer 池与剧本 `dialogue` 事件复用。
/// `overrides` 让固定台词覆盖响应字段（显示名/副标题/时长），生成路径传默认值。
///
/// 本入口按「调用即一轮」就地捕获角色快照，供没有跨轮快照的调用方使用；
/// 生成路径已持有整轮快照，直接走 `consume_sentence_with_snapshot`。
pub(crate) async fn consume_sentence(
    deps: &SentenceDeps,
    sentence: String,
    user_message: &str,
    is_final: bool,
    user_message_seq: Option<u32>,
    thinking_buf: &Mutex<String>,
    overrides: &ReplyOverrides,
) -> Result<Option<ReplyResponse>> {
    let snapshot = RoundRoleSnapshot::capture(deps).await;
    consume_sentence_with_snapshot(
        deps,
        &snapshot,
        sentence,
        user_message,
        is_final,
        user_message_seq,
        thinking_buf,
        overrides,
    )
    .await
}

/// `consume_sentence` 的整轮快照版本：角色归属与音色一律取本轮快照。
async fn consume_sentence_with_snapshot(
    deps: &SentenceDeps,
    snapshot: &RoundRoleSnapshot,
    sentence: String,
    user_message: &str,
    is_final: bool,
    user_message_seq: Option<u32>,
    thinking_buf: &Mutex<String>,
    overrides: &ReplyOverrides,
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
    enrich_segments(deps, snapshot, &mut segments).await?;

    // 3. 构建前端响应
    let mut response = build_reply_response(
        deps,
        snapshot,
        &segments,
        user_message,
        is_final,
        user_message_seq,
        overrides,
    )
    .await?;

    // 3.5 最终句：快照本轮思考链，挂载到响应与台词行（供历史对话展示思考过程）
    if is_final {
        let thinking = thinking_buf.lock().await;
        if !thinking.is_empty() {
            response.thinking = Some(thinking.clone());
        }
    }

    // 4. 写入 GameStatus
    add_assistant_line(deps, &mut response).await?;

    // 会话代号复核（与 add_assistant_line 内守卫一致）：若本句写入已被丢弃，
    // 说明读档/切角色/清对话已切换会话（旧流式任务游离），前端也已切到新会话——
    // 这里返回 None，不再把这条过期回复发布给前端（防旧角色台词串进新对话展示）。
    {
        let gs = deps.game_status.lock().await;
        if gs.preview_generation != deps.generation {
            return Ok(None);
        }
    }

    Ok(Some(response))
}

/// Step A: 解析并分类情绪片段。
fn parse_segments(deps: &SentenceDeps, sentence: &str) -> Vec<EmotionSegment> {
    let segments = deps
        .processor
        .parse_and_classify_emotional_segments(sentence);
    if segments.is_empty() {
        tracing::warn!("AI 回复格式错误（未找到情绪 tag）");
    }
    segments
}

/// 返回当前 TTS 需要的目标翻译语言。
fn tts_translation_language(tts_type: &str, voice_lang: &str) -> Option<&'static str> {
    match (tts_type, voice_lang) {
        ("gsv" | "opentts" | "sbv2" | "fishs2", "en") => Some("en"),
        // IndexTTS 2.5 起官方支持中/英/日/西班牙/阿拉伯语：
        // voice_lang 为非中文目标语言时先翻译成对应语言再合成
        // （日语台词主模型已自带 japanese_text，无需在此强制重译）
        ("indextts2", "en") => Some("en"),
        ("indextts2", "es") => Some("es"),
        ("indextts2", "ar") => Some("ar"),
        ("gsv" | "opentts", "ko") => Some("ko"),
        // CosyVoice 多语言自动检测：voice_lang 为 en/ko/de/fr/ru/pt 时先翻译成目标语言
        // 再合成，否则会朗读主模型默认附带的日文译文（japanese_text）；
        // ja 例外——主模型已自带日文译文，无需重译
        ("cosyvoice", "en") => Some("en"),
        ("cosyvoice", "ko") => Some("ko"),
        ("cosyvoice", "de") => Some("de"),
        ("cosyvoice", "fr") => Some("fr"),
        ("cosyvoice", "ru") => Some("ru"),
        ("cosyvoice", "pt") => Some("pt"),
        _ => None,
    }
}

/// 判断文本是否适合作为日语 TTS 输入。
fn looks_like_japanese(text: &str) -> bool {
    let has_kana = text
        .chars()
        .any(|c| matches!(c, '\u{3040}'..='\u{30ff}' | '\u{31f0}'..='\u{31ff}'));
    let has_cjk = text
        .chars()
        .any(|c| matches!(c, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'));
    let has_ascii_letters = text.chars().any(|c| c.is_ascii_alphabetic());

    has_kana || (has_cjk && !has_ascii_letters)
}

/// 切回日语后，检测并修复上一次英语模式残留的译文。
fn needs_japanese_translation(segments: &[EmotionSegment]) -> bool {
    segments.iter().any(|segment| {
        !segment.following_text.trim().is_empty()
            && !looks_like_japanese(segment.japanese_text.trim())
    })
}

/// Step B: 翻译与语音生成。
///
/// 音色字段一律取本轮快照：翻译与语音合成都发生在本步之后的长 await 中，
/// 若此处再读 `current_role_id`，期间的角色切换会让台词用上别人的音色。
async fn enrich_segments(
    deps: &SentenceDeps,
    snapshot: &RoundRoleSnapshot,
    segments: &mut [EmotionSegment],
) -> Result<()> {
    let translation_language = tts_translation_language(&snapshot.tts_type, &snapshot.voice_lang)
        .or_else(|| {
            if snapshot.voice_lang == "ja" && needs_japanese_translation(segments) {
                Some("ja")
            } else {
                None
            }
        });

    if let Some(target_lang) = translation_language {
        let translated = deps
            .translator
            .translate_segments_to(segments, true, target_lang)
            .await?;
        if !translated {
            // 目标语言翻译失败时不要回退朗读主模型附带的其他语言译文。
            for segment in segments.iter_mut() {
                segment.japanese_text.clear();
            }
        }
    }

    if let Some(vm) = &snapshot.voice_maker {
        vm.generate_voice_files(segments).await;
    }

    Ok(())
}

/// Step C: 构建 ReplyResponse（含角色信息填充）。
///
/// 归属角色取本轮快照；同时对比此刻的 `GameStatus.current_role_id`，
/// 不一致只告警不改归属——本轮从生成到落地都必须归属同一个角色。
async fn build_reply_response(
    deps: &SentenceDeps,
    snapshot: &RoundRoleSnapshot,
    segments: &[EmotionSegment],
    user_message: &str,
    is_final: bool,
    user_message_seq: Option<u32>,
    overrides: &ReplyOverrides,
) -> Result<ReplyResponse> {
    // 角色漂移观察位：本轮开始后若有别的写路径（工具调用、附身切换等）改了当前角色，
    // 这里能留下证据，但仍按快照归属，避免同一轮正文被算到另一个角色名下。
    {
        let gs = deps.game_status.lock().await;
        if gs.current_role_id != snapshot.role_id {
            tracing::warn!(
                "本轮角色快照与当前角色不一致（快照 {:?}，当前 {:?}），回复仍归属快照角色",
                snapshot.role_id,
                gs.current_role_id
            );
        }
    }

    let first = &segments[0];
    // 快照没有角色时回退到情绪分段自带的角色信息；有快照则保留 role_id，
    // 让角色未加载（如工具刚切换）时前端也能按 role_id 自行加载，而不是丢成 None
    let (character, role_id) = match snapshot.role_id {
        Some(rid) => (
            snapshot.display_name.clone().or(first.character.clone()),
            Some(rid),
        ),
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
    // 试玩标记：前端据此丢弃中止后迟到的流式回复（非试玩为 None，不序列化）
    response.preview_gen = if deps.is_preview {
        Some(deps.generation)
    } else {
        None
    };

    // 固定台词覆盖：dialogue 事件传入显示名/副标题/时长，生成路径全为默认值
    if let Some(dn) = &overrides.display_name {
        response.display_name = Some(dn.clone());
    }
    if let Some(ds) = &overrides.display_subtitle {
        response.display_subtitle = Some(ds.clone());
    }
    response.duration = overrides.duration.unwrap_or(-1.0);

    Ok(response)
}

/// Step D: 将 assistant LINE 写入 GameStatus，并回填该行随 `ai:reply` 下发的 TTS 序号。
async fn add_assistant_line(deps: &SentenceDeps, response: &mut ReplyResponse) -> Result<()> {
    // 试玩代号守卫：试玩任务被中止后，游离的 consumer 任务仍会带着旧代号继续
    // 生成句子。此时 GameStatus 可能已还原回自由对话，写入会把试玩台词漏进
    // 自由对话的上下文与历史。捕获代号与当前值不一致即丢弃整条（含记忆同步）。
    {
        let gs = deps.game_status.lock().await;
        if gs.preview_generation != deps.generation {
            tracing::warn!(
                "[Generator] 丢弃过期试玩回复（代号 {} != 当前 {}），试玩已结束",
                deps.generation,
                gs.preview_generation
            );
            return Ok(());
        }
    }
    // sender_role_id / display_name 均来自 response：角色归属已由本轮快照透传进来，
    // 此处不再读 GameStatus.current_role_id，避免与生成时的角色不一致
    let line = LineBase {
        content: response.message.clone(),
        sender_role_id: response.role_id,
        original_emotion: Some(response.original_tag.clone()),
        predicted_emotion: Some(response.emotion.clone()),
        tts_content: response.tts_text.clone(),
        action_content: response.motion_text.clone(),
        audio_file: response.audio_file.clone(),
        thinking: response.thinking.clone(),
        // 优先使用覆盖的显示名（dialogue 事件），生成路径 display_name 为 None 时回退角色名
        display_name: response.display_name.clone().or(response.character.clone()),
        attribute: LineAttributeExt(LineAttribute::Assistant),
        ..Default::default()
    };
    let mut gs = deps.game_status.lock().await;
    gs.add_line(&deps.db, line).await?;
    // 行落库后再取号，保证序号与该行在 line_list 中的实际位置一致；
    // 前端回传这个序号调 generate_line_voice，无需自行计数。
    response.tts_seq =
        crate::api::chat::tts_seq_at(&gs.line_list, gs.line_list.len().saturating_sub(1));
    Ok(())
}
