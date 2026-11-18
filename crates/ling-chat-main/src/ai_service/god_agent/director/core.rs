//! 导演 Agent 运行层：存档懒同步、现场装配、流式对话。

use std::collections::HashSet;

use chrono::Local;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::core::tail_lines;
use crate::ai_service::llm::{LlmChunk, LlmClient, LlmSlot, LlmUsage, slot_snapshot};
use crate::ai_service::skill_agent::db;
use crate::ai_service::skill_agent::events::Usage as AgentUsage;
use crate::ai_service::tools::executor::{ToolContext, ToolExecutor};
use crate::ai_service::types::{GameLine, LlmMessage, ToolCall, ToolDefinition};
use crate::db::entities::agent_message;
use crate::db::entities::line::LineAttribute;

use super::draft::{
    DRAFT_INSTRUCTION, DRAFT_MAX_ATTEMPTS, DRAFT_USER_PROMPT, DirectorConfirmResult,
    DirectorDraftView, DirectorModeState, DirectorModeView, draft_event_catalog, parse_draft,
};
use super::tools::{
    LoopStep, MAX_TOOL_CALLS_PER_ROUND, director_registry, plan_next_step,
};
use super::{
    ContextMessage, ContextSection, ContextStats, DirectorAgentEvent, DirectorContext,
    DirectorMessage,
};

/// 导演人设。写死在代码里，本期不进 prompts 目录。
const DIRECTOR_PERSONA: &str = "你是这场对话的导演，站在舞台之外观察现场，负责安排剧情走向。\n\
玩家会在这里告诉你想看到什么，你可以据此调整场景、氛围与角色的处境。\n\
你看得到下面列出的现场信息与最近的对话，但不要代替角色说话，也不要替玩家做决定。";

/// 单条台词进入 system 时的字符上限。
const LINE_CHAR_LIMIT: usize = 240;
/// 角色简介的字符上限。
const ROLE_INFO_LIMIT: usize = 200;
/// 「最近对话」整段的字符上限。
const RECENT_BLOCK_LIMIT: usize = 6000;
/// system 总字符上限。
const SYSTEM_CHAR_LIMIT: usize = 10000;
/// 会话历史进入请求的字符预算。
const HISTORY_CHAR_LIMIT: usize = 12000;
/// 最多列出多少位在场角色。
const MAX_LISTED_ROLES: usize = 12;

/// 装配好的 system：整段文本 + 分段视图 + 是否截断。
pub struct AssembledSystem {
    pub prompt: String,
    pub sections: Vec<ContextSection>,
    pub truncated: bool,
}

/// 一轮对话的落库与收尾凭据。
pub struct TurnContext {
    /// 用户消息的 DB id；0 表示自由对话未落库。
    pub user_id: i32,
    /// 本轮写入的导演会话 id；切档后仍按它落库。
    pub conversation_id: Option<i32>,
    pub cancel: CancellationToken,
}

/// 导演会话状态。与 `GameStatus.active_save_id` 不一致时懒重载。
#[derive(Default)]
pub struct DirectorSession {
    /// 已同步的存档 ID。
    pub save_id: Option<i32>,
    /// 该存档对应的导演会话行 id；自由对话时为 None。
    pub conversation_id: Option<i32>,
    pub messages: Vec<DirectorMessage>,
    /// 上次真正发给 LLM 的上下文快照，供「查看记忆」展示。
    pub last_context: Option<DirectorContext>,
    pub running: bool,
    /// 本轮取消令牌（`director_stop_chat` 触发）。
    pub cancel: Option<CancellationToken>,
    /// 当前运行的任务句柄（善后用，不靠 abort 停）。
    pub task: Option<tauri::async_runtime::JoinHandle<()>>,
    /// 周期小剧本模式：开关、轮次进度与待确认草案（只在内存，随切档重置）。
    pub mode: DirectorModeState,
}

pub struct DirectorAgent {
    /// 与上帝 Agent 共用同一槽位，热切换 LLM 自动生效。
    pub llm: LlmSlot,
    /// 会话状态用 Arc 包一层，周期草案后台任务能安全持有并写回。
    pub session: std::sync::Arc<tokio::sync::Mutex<DirectorSession>>,
}

impl DirectorAgent {
    pub fn new(llm: LlmSlot) -> Self {
        Self {
            llm,
            session: std::sync::Arc::new(tokio::sync::Mutex::new(DirectorSession::default())),
        }
    }

    // ============================================================
    // 会话状态
    // ============================================================

    /// 把会话同步到当前存档。
    ///
    /// None 到 Some 是常态路径：自动存档会在自由对话里建档并绑定存档，
    /// 已聊的消息要收编进新存档，而不是凭空消失。
    pub async fn sync_save(
        &self,
        db: &DatabaseConnection,
        save_id: Option<i32>,
    ) -> Result<(), String> {
        let mut session = self.session.lock().await;
        Self::sync_locked(&mut session, db, save_id).await
    }

    async fn sync_locked(
        session: &mut DirectorSession,
        db: &DatabaseConnection,
        save_id: Option<i32>,
    ) -> Result<(), String> {
        if session.save_id == save_id {
            return Ok(());
        }
        // 现场可能整体换过，旧快照不再代表下一次请求
        session.last_context = None;
        // 轮次进度属于旧存档，草案也不该跨档带过去（开关与间隔是用户偏好，保留）
        session.mode.reset_progress();

        match save_id {
            None => {
                session.save_id = None;
                session.conversation_id = None;
                session.messages.clear();
            },
            Some(new_id) => {
                // 自由对话转成有档：内存里已聊的消息收编进该档的导演线
                let adopts_memory = session.save_id.is_none() && !session.messages.is_empty();
                let conversation_id = if adopts_memory {
                    Some(db::get_or_create_director_conversation(db, new_id).await?)
                } else {
                    // 还没聊过就不建空会话，首次发言时再建
                    db::latest_director_conversation(db, new_id)
                        .await?
                        .map(|conversation| conversation.id)
                };
                if adopts_memory {
                    if let Some(conversation_id) = conversation_id {
                        for message in &session.messages {
                            db::append_director_message(
                                db,
                                conversation_id,
                                &message.to_llm(),
                                message.reasoning.as_deref(),
                                None,
                            )
                            .await?;
                        }
                    }
                }
                session.save_id = Some(new_id);
                session.conversation_id = conversation_id;
                session.messages = match conversation_id {
                    Some(id) => load_messages(db, id).await?,
                    None => Vec::new(),
                };
            },
        }
        Ok(())
    }

    /// 侧栏 / 切档后拉取的会话快照。
    pub async fn state_snapshot(&self) -> (Option<i32>, bool, Vec<DirectorMessage>) {
        let session = self.session.lock().await;
        (session.save_id, session.running, session.messages.clone())
    }

    pub async fn last_context(&self) -> Option<DirectorContext> {
        self.session.lock().await.last_context.clone()
    }

    pub async fn is_running(&self) -> bool {
        self.session.lock().await.running
    }

    /// 工具调用历史：带 tool_calls 的 assistant 消息与 tool 结果消息。
    pub async fn tool_history(&self) -> Vec<DirectorMessage> {
        let session = self.session.lock().await;
        session
            .messages
            .iter()
            .filter(|m| m.role == "tool" || m.tool_calls.is_some())
            .cloned()
            .collect()
    }

    /// 把本轮任务句柄交给会话持有（停止时 await 它等落库完成）。
    pub async fn set_task(&self, task: tauri::async_runtime::JoinHandle<()>) {
        self.session.lock().await.task = Some(task);
    }

    /// 取走取消令牌与任务句柄，交给停止命令。
    pub async fn take_control(
        &self,
    ) -> (
        Option<CancellationToken>,
        Option<tauri::async_runtime::JoinHandle<()>>,
    ) {
        let mut session = self.session.lock().await;
        (session.cancel.take(), session.task.take())
    }

    // ============================================================
    // 周期小剧本模式（D4）
    // ============================================================

    /// 主聊天一轮回复落库后的轻量触发检查。
    ///
    /// 达到间隔时原子占住生成位并起后台任务，不阻塞主回复；
    /// 剧本运行 / 编辑器试玩期间不生成（D1 闸门由调用方判定）。
    /// god_agent 不可用时占住生成位的草案直接落 failed，给出明确错误。
    pub async fn maybe_generate_draft(&self, app: AppHandle, tools_enabled: bool) {
        if !tools_enabled {
            return;
        }
        let token = {
            let mut session = self.session.lock().await;
            if !session.mode.enabled {
                return;
            }
            // 台词表折算出的当前最大用户消息序号
            let current_seq = {
                let service = app.state::<crate::AppState>().ai_service.lock().await;
                let gs = service.game_status.lock().await;
                crate::api::game::compute_user_message_seqs(&gs.line_list)
                    .iter()
                    .filter_map(|seq| *seq)
                    .max()
                    .unwrap_or(0)
            };
            session.mode.begin_generation(current_seq)
        };
        let Some(token) = token else { return };

        let state = app.state::<crate::AppState>();
        let Some(core) = state.god_agent.clone() else {
            self.finish_draft(token, Err("导演服务不可用：未配置可用的对话模型".to_string()))
                .await;
            return;
        };
        let llm = match slot_snapshot(&core.llm).await {
            Some(llm) => llm,
            None => {
                self.finish_draft(token, Err("导演没有可用的对话模型".to_string()))
                    .await;
                return;
            },
        };
        let window = core.config_snapshot().recent_window;
        let assembled = {
            let service = state.ai_service.lock().await;
            let gs = service.game_status.lock().await;
            Self::assemble(&gs, window)
        };
        let session = self.session.clone();
        tauri::async_runtime::spawn(async move {
            let result = generate_draft(llm.as_ref(), assembled).await;
            let mut session = session.lock().await;
            session.mode.finish_generation(token, result);
        });
    }

    /// 模式视图，供 director_get_mode / director_set_mode 返回。
    pub async fn mode_view(&self) -> DirectorModeView {
        self.session.lock().await.mode.mode_view()
    }

    pub async fn draft_view(&self) -> DirectorDraftView {
        self.session.lock().await.mode.draft_view()
    }

    pub async fn set_mode(&self, enabled: bool, interval: Option<usize>) -> DirectorModeView {
        let mut session = self.session.lock().await;
        session.mode.set_mode(enabled, interval);
        session.mode.mode_view()
    }

    pub async fn discard_draft(&self) -> DirectorDraftView {
        let mut session = self.session.lock().await;
        session.mode.discard();
        session.mode.draft_view()
    }

    /// 确认草案：pending 态取走事件，逐个走 S2 同一执行路径；
    /// 全部成功才推进 last_generated_seq（期间 applying 态天然防重）。
    pub async fn confirm_draft(&self, app: AppHandle) -> Result<DirectorConfirmResult, String> {
        let events = {
            let mut session = self.session.lock().await;
            session
                .mode
                .begin_confirm()
                .ok_or_else(|| "当前没有待确认的草案".to_string())?
        };
        let mut applied = 0usize;
        let mut failure: Option<String> = None;
        for (index, item) in events.iter().enumerate() {
            let type_key = item["event"].as_str().unwrap_or_default().to_string();
            let params = item.get("params").cloned().unwrap_or_else(|| json!({}));
            if let Err(error) =
                super::event_tools::execute_director_event(&app, &type_key, params).await
            {
                failure = Some(format!("第 {} 个事件 {type_key}：{error}", index + 1));
                break;
            }
            applied += 1;
        }
        let mut session = self.session.lock().await;
        session.mode.finish_confirm(applied, failure.clone());
        drop(session);
        // 命令层统一用 Ok(结果) 把成败交给前端结果条，失败不再走 Err 丢掉已投递数。
        Ok(DirectorConfirmResult {
            ok: failure.is_none(),
            applied,
            error: failure,
        })
    }

    async fn finish_draft(&self, token: u64, result: Result<Vec<Value>, String>) {
        let mut session = self.session.lock().await;
        session.mode.finish_generation(token, result);
    }

    // ============================================================
    // 一轮对话的准备
    // ============================================================

    /// 同步存档、落库用户消息、占住运行位，返回本轮凭据。
    pub async fn begin_turn(
        &self,
        db: &DatabaseConnection,
        save_id: Option<i32>,
        text: &str,
    ) -> Result<TurnContext, String> {
        let mut session = self.session.lock().await;
        if session.running {
            return Err("导演正在生成回复，请稍候".to_string());
        }
        Self::sync_locked(&mut session, db, save_id).await?;

        let (conversation_id, user_id) = match session.save_id {
            Some(active) => {
                let conversation_id = db::get_or_create_director_conversation(db, active).await?;
                session.conversation_id = Some(conversation_id);
                let id = db::append_director_message(
                    db,
                    conversation_id,
                    &LlmMessage::user(text),
                    None,
                    None,
                )
                .await?;
                (Some(conversation_id), id)
            },
            None => (None, 0),
        };

        session.messages.push(DirectorMessage {
            id: user_id,
            role: "user".to_string(),
            content: text.to_string(),
            reasoning: None,
            tool_calls: None,
            tool_call_id: None,
            created_at: Local::now().naive_local().to_string(),
        });
        let cancel = CancellationToken::new();
        session.cancel = Some(cancel.clone());
        session.running = true;

        Ok(TurnContext {
            user_id,
            conversation_id,
            cancel,
        })
    }

    /// 用装配好的 system 拼出本轮请求，并把快照存进会话。
    pub async fn build_request(
        &self,
        assembled: AssembledSystem,
        source: &str,
        provider: Option<String>,
        model: Option<String>,
    ) -> (Vec<LlmMessage>, DirectorContext) {
        let mut session = self.session.lock().await;
        let (request, context) =
            compose_request(&session.messages, assembled, source, provider, model);
        session.last_context = Some(context.clone());
        (request, context)
    }

    /// 按当前现场重新装配一份快照（不覆盖上次实际请求的那份）。
    pub async fn rebuild_context(
        &self,
        assembled: AssembledSystem,
        provider: Option<String>,
        model: Option<String>,
    ) -> DirectorContext {
        let session = self.session.lock().await;
        let (_request, context) =
            compose_request(&session.messages, assembled, "rebuilt", provider, model);
        context
    }

    /// 清空当前存档的导演线（DB + 内存）。
    pub async fn clear(&self, db: &DatabaseConnection, save_id: Option<i32>) -> Result<(), String> {
        let mut session = self.session.lock().await;
        if session.running {
            return Err("导演正在生成回复，请稍候".to_string());
        }
        Self::sync_locked(&mut session, db, save_id).await?;
        if let Some(conversation_id) = session.conversation_id {
            db::clear_messages(db, conversation_id).await?;
        }
        session.messages.clear();
        session.last_context = None;
        Ok(())
    }

    // ============================================================
    // 流式运行
    // ============================================================

    /// 跑一轮对话。自由对话时带导演工具定义跑多轮闭环，正文边流边推；
    /// 结束、停止、出错都把已产出的内容与工具消息落库。
    ///
    /// `tools_enabled` 为 D1 闸门结果：剧本运行期间为 false，不下发工具定义，
    /// 模型照历史硬调时执行层仍返回明确拒绝文案。
    pub async fn run_chat(
        &self,
        db: DatabaseConnection,
        channel: Channel<DirectorAgentEvent>,
        request: Vec<LlmMessage>,
        turn: TurnContext,
        app: AppHandle,
        tools_enabled: bool,
    ) {
        let Some(llm) = slot_snapshot(&self.llm).await else {
            let _ = channel.send(DirectorAgentEvent::Error {
                message: "导演没有可用的对话模型".to_string(),
            });
            self.finish_turn(&db, &turn, String::new(), String::new(), None)
                .await;
            return;
        };

        // 注册表与聊天工具权限体系完全隔离，这里现建现用。
        let registry = director_registry();
        let streaming_tools = tools_enabled && llm.supports_streaming_tools();
        let definitions: Vec<ToolDefinition> = if streaming_tools {
            registry.definitions()
        } else {
            Vec::new()
        };
        let allowed: HashSet<String> = definitions
            .iter()
            .map(|definition| definition.function.name.clone())
            .collect();
        let context = ToolContext::new(allowed).with_app(app);
        let executor = ToolExecutor::new(&registry);

        let mut messages = request;
        let cancel = turn.cancel.clone();
        let mut usage: Option<LlmUsage> = None;
        let mut error: Option<String> = None;
        let mut content = String::new();
        let mut reasoning = String::new();
        let mut round = 0usize;
        let mut final_synthesis = false;
        let mut stopped = false;

        loop {
            // 收尾轮与闸门关闭时都不下发工具定义，避免模型继续调工具。
            let response = if !final_synthesis && !definitions.is_empty() {
                llm.complete_stream_with_tools(&messages, &definitions, Some("auto"))
                    .await
            } else {
                llm.complete_stream(&messages).await
            };
            let mut stream = match response {
                Err(e) => {
                    error = Some(format!("导演请求失败：{e}"));
                    break;
                },
                Ok(stream) => stream,
            };

            let mut round_content = String::new();
            let mut round_reasoning = String::new();
            let mut round_calls: Vec<ToolCall> = Vec::new();
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        let _ = channel.send(DirectorAgentEvent::Status {
                            content: "已停止生成".to_string(),
                        });
                        stopped = true;
                        break;
                    }
                    chunk = stream.next() => {
                        match chunk {
                            None => break,
                            Some(Err(e)) => {
                                error = Some(e.to_string());
                                break;
                            }
                            Some(Ok(chunk)) => match chunk {
                                LlmChunk::Content(c) => {
                                    round_content.push_str(&c);
                                    let _ = channel.send(DirectorAgentEvent::MessageDelta { content: c });
                                }
                                LlmChunk::Reasoning(r) => {
                                    round_reasoning.push_str(&r);
                                    let _ = channel.send(DirectorAgentEvent::Reasoning { content: r });
                                }
                                LlmChunk::ToolCalls(calls) => round_calls.extend(calls),
                                LlmChunk::StreamEnd { usage: end, .. } => {
                                    if end.is_some() {
                                        usage = end;
                                    }
                                }
                                LlmChunk::ToolCallProgress { .. } => {}
                            },
                        }
                    }
                }
            }

            // 各轮正文都实时推给前端，最终答复也按同样顺序累加，
            // 保证落库内容、Done.final_text 与前端流式累积三者一致。
            content.push_str(&round_content);
            reasoning.push_str(&round_reasoning);
            if error.is_some() || stopped {
                break;
            }

            // 收尾轮只取正文：模型残留的工具调用一律忽略，不再进闭环。
            if final_synthesis {
                if !round_calls.is_empty() {
                    let count = round_calls.len();
                    tracing::warn!(count, "导演收尾轮仍返回工具调用，已忽略");
                }
                break;
            }
            if round_calls.is_empty() {
                break;
            }

            // 工具执行前先把带 tool_calls 的 assistant 消息按序落库。
            let mut assistant_msg = LlmMessage::tool(round_calls.clone());
            assistant_msg.content = round_content.clone();
            self.append_loop_message(
                &db,
                &turn,
                &assistant_msg,
                nonempty(&round_reasoning),
            )
            .await;
            messages.push(assistant_msg);

            if !streaming_tools {
                // 没下发工具定义却仍收到工具调用：逐个回明确文案，再给一次
                // 不带工具定义的收尾，让导演用一句话解释现状。
                let reason = if tools_enabled {
                    "当前对话模型不支持原生工具调用，本次调用未执行。"
                } else {
                    "当前正在运行固定剧本，导演工具已关闭；请结束剧本后再安排剧情。"
                };
                for call in &round_calls {
                    let _ = channel.send(DirectorAgentEvent::ToolCall {
                        call_id: call.id.clone(),
                        tool: call.function.name.clone(),
                        args: parse_tool_args(&call.function.arguments),
                    });
                    let output = json!({
                        "ok": false,
                        "error": {
                            "code": "tools_disabled",
                            "message": reason
                        }
                    })
                    .to_string();
                    let _ = channel.send(DirectorAgentEvent::ToolResult {
                        call_id: call.id.clone(),
                        tool: call.function.name.clone(),
                        ok: false,
                        output: output.clone(),
                        error: Some(reason.to_string()),
                    });
                    let tool_msg = LlmMessage::tool_result(call.id.clone(), output);
                    self.append_loop_message(&db, &turn, &tool_msg, None).await;
                    messages.push(tool_msg);
                }
                final_synthesis = true;
                round += 1;
                continue;
            }

            // 逐个执行工具；取消后剩余调用补「已取消」结果，避免留下孤立的 tool_calls。
            for (index, call) in round_calls.iter().enumerate() {
                let _ = channel.send(DirectorAgentEvent::ToolCall {
                    call_id: call.id.clone(),
                    tool: call.function.name.clone(),
                    args: parse_tool_args(&call.function.arguments),
                });
                let output = if cancel.is_cancelled() {
                    stopped = true;
                    json!({
                        "ok": false,
                        "error": {
                            "code": "cancelled",
                            "message": "生成已停止，该工具调用未执行。"
                        }
                    })
                    .to_string()
                } else if index >= MAX_TOOL_CALLS_PER_ROUND {
                    json!({
                        "ok": false,
                        "error": {
                            "code": "call_limit",
                            "message": "本轮工具调用数超过上限，该调用未执行，请下一轮再试。"
                        }
                    })
                    .to_string()
                } else {
                    executor
                        .execute(&call.function.name, &call.function.arguments, &context)
                        .await
                };
                let (ok, tool_error) = parse_tool_outcome(&output);
                let _ = channel.send(DirectorAgentEvent::ToolResult {
                    call_id: call.id.clone(),
                    tool: call.function.name.clone(),
                    ok,
                    output: output.clone(),
                    error: tool_error,
                });
                let tool_msg = LlmMessage::tool_result(call.id.clone(), output);
                self.append_loop_message(&db, &turn, &tool_msg, None).await;
                messages.push(tool_msg);
            }
            if stopped {
                let _ = channel.send(DirectorAgentEvent::Status {
                    content: "已停止生成".to_string(),
                });
                break;
            }

            match plan_next_step(round, true) {
                LoopStep::Finish => break,
                LoopStep::ContinueWithTools => round += 1,
                LoopStep::FinalSynthesis => {
                    // 用满工具轮数：下一轮不带工具定义做一次收尾生成。
                    final_synthesis = true;
                    round += 1;
                },
            }
        }

        let message_id = self
            .finish_turn(&db, &turn, content.clone(), reasoning, usage)
            .await;
        match error {
            Some(message) => {
                let _ = channel.send(DirectorAgentEvent::Error { message });
            },
            None => {
                let _ = channel.send(DirectorAgentEvent::Done {
                    final_text: content,
                    message_id,
                });
            },
        }
    }

    /// 工具循环中间消息：assistant(tool_calls) 与 tool 结果按序写进当前会话。
    /// 自由对话未落库时只进内存 session.messages，与 finish_turn 一致。
    async fn append_loop_message(
        &self,
        db: &DatabaseConnection,
        turn: &TurnContext,
        message: &LlmMessage,
        reasoning: Option<&str>,
    ) {
        let mut id = 0;
        if let Some(conversation_id) = turn.conversation_id {
            match db::append_director_message(db, conversation_id, message, reasoning, None).await {
                Ok(new_id) => id = new_id,
                Err(e) => tracing::warn!("导演工具消息落库失败: {e}"),
            }
        }

        let mut session = self.session.lock().await;
        // 生成期间切了存档：消息已写进旧存档的会话，内存里只保留新档的线。
        if session.conversation_id == turn.conversation_id {
            session.messages.push(DirectorMessage {
                id,
                role: message.role.clone(),
                content: message.content.clone(),
                reasoning: reasoning.map(str::to_string),
                tool_calls: message.tool_calls.clone(),
                tool_call_id: message.tool_call_id.clone(),
                created_at: Local::now().naive_local().to_string(),
            });
        }
    }

    /// 收尾：落库 assistant 回复、写回会话、清掉运行位。返回消息 id。
    async fn finish_turn(
        &self,
        db: &DatabaseConnection,
        turn: &TurnContext,
        content: String,
        reasoning: String,
        usage: Option<LlmUsage>,
    ) -> i32 {
        let has_output = !content.is_empty() || !reasoning.is_empty();
        let reasoning_opt = (!reasoning.is_empty()).then_some(reasoning);
        let mut message_id = 0;

        if has_output {
            if let Some(conversation_id) = turn.conversation_id {
                let usage = usage.map(to_agent_usage);
                match db::append_director_message(
                    db,
                    conversation_id,
                    &LlmMessage::assistant(&content),
                    reasoning_opt.as_deref(),
                    usage.as_ref(),
                )
                .await
                {
                    Ok(id) => message_id = id,
                    Err(e) => tracing::warn!("导演回复落库失败: {e}"),
                }
            }
        }

        let mut session = self.session.lock().await;
        session.running = false;
        session.cancel = None;
        // 生成期间切了存档：消息已写进旧存档的会话，内存里只保留新档的线
        if has_output && session.conversation_id == turn.conversation_id {
            session.messages.push(DirectorMessage {
                id: message_id,
                role: "assistant".to_string(),
                content,
                reasoning: reasoning_opt,
                tool_calls: None,
                tool_call_id: None,
                created_at: Local::now().naive_local().to_string(),
            });
        }
        message_id
    }

    // ============================================================
    // 上下文装配
    // ============================================================

    /// 装配导演本轮看到的 system：人设 + 当前现场 + 在场角色 + 最近对话。
    pub fn assemble(gs: &GameStatus, window: usize) -> AssembledSystem {
        let mut remaining = SYSTEM_CHAR_LIMIT;
        let mut truncated = false;
        let mut sections = Vec::new();

        push_section(
            &mut sections,
            "persona",
            "导演人设",
            DIRECTOR_PERSONA.to_string(),
            &mut remaining,
            &mut truncated,
        );
        push_section(
            &mut sections,
            "scene",
            "当前现场",
            build_scene(gs),
            &mut remaining,
            &mut truncated,
        );
        push_section(
            &mut sections,
            "roles",
            "在场角色",
            build_roles(gs),
            &mut remaining,
            &mut truncated,
        );

        let (recent, recent_truncated) =
            build_recent(gs, window, remaining.min(RECENT_BLOCK_LIMIT));
        truncated |= recent_truncated;
        push_section(
            &mut sections,
            "recent",
            "最近对话",
            recent,
            &mut remaining,
            &mut truncated,
        );

        let prompt = sections
            .iter()
            .map(|s| s.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        AssembledSystem {
            prompt,
            sections,
            truncated,
        }
    }
}

/// 空 reasoning 不写进 DB（与 finish_turn 一致）。
fn nonempty(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

/// 工具事件的参数摘要：非法或双编码 JSON 退化成空对象，保证前端字段始终是对象。
fn parse_tool_args(raw: &str) -> Value {
    serde_json::from_str::<Value>(raw)
        .ok()
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// 从执行器返回串判断成败，并抽出错误文案供前端 ToolResult 展示。
fn parse_tool_outcome(output: &str) -> (bool, Option<String>) {
    let value = serde_json::from_str::<Value>(output).ok();
    let ok = value
        .as_ref()
        .and_then(|v| v.get("ok"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let error = if ok {
        None
    } else {
        value
            .as_ref()
            .and_then(|v| v.pointer("/error/message"))
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    (ok, error)
}

fn to_agent_usage(usage: LlmUsage) -> AgentUsage {
    AgentUsage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        cached_tokens: usage.cached_tokens,
    }
}

/// 从 DB 读回该会话的全部消息。
async fn load_messages(
    db: &DatabaseConnection,
    conversation_id: i32,
) -> Result<Vec<DirectorMessage>, String> {
    Ok(db::list_messages(db, conversation_id)
        .await?
        .iter()
        .map(model_to_message)
        .collect())
}

fn model_to_message(m: &agent_message::Model) -> DirectorMessage {
    DirectorMessage {
        id: m.id,
        role: m.role.clone(),
        content: m.content.clone().unwrap_or_default(),
        reasoning: m.reasoning.clone(),
        tool_calls: m
            .tool_calls
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok()),
        tool_call_id: m.tool_call_id.clone(),
        created_at: m.created_at.to_string(),
    }
}

/// 用导演 LLM 槽位生成草案：system 复用 DirectorAgent::assemble 的现场上下文，
/// 追加严格 JSON 指令与 8 类事件的参数目录。解析 / 校验失败重试，仍失败返回明确错误。
async fn generate_draft(llm: &LlmClient, assembled: AssembledSystem) -> Result<Vec<Value>, String> {
    let system = format!(
        "{}\n\n{}\n\n{}",
        assembled.prompt,
        DRAFT_INSTRUCTION,
        draft_event_catalog()
    );
    let messages = vec![
        LlmMessage::system(system),
        LlmMessage::user(DRAFT_USER_PROMPT),
    ];
    let mut last_error = String::new();
    for attempt in 1..=DRAFT_MAX_ATTEMPTS {
        let raw = match llm.complete(&messages).await {
            Ok(raw) => raw,
            Err(error) => {
                last_error = format!("导演模型请求失败：{error}");
                tracing::warn!(attempt, "{last_error}");
                continue;
            },
        };
        match parse_draft(&raw) {
            Ok(events) => return Ok(events),
            Err(error) => {
                last_error = error;
                tracing::warn!(attempt, "草案解析或校验失败：{last_error}");
            },
        }
    }
    Err(format!(
        "草案生成失败（已重试 {DRAFT_MAX_ATTEMPTS} 次）：{last_error}"
    ))
}

fn compose_request(
    messages: &[DirectorMessage],
    assembled: AssembledSystem,
    source: &str,
    provider: Option<String>,
    model: Option<String>,
) -> (Vec<LlmMessage>, DirectorContext) {
    let history: Vec<LlmMessage> = messages.iter().map(DirectorMessage::to_llm).collect();
    let sanitized = sanitize_tool_pairing(&history);
    let (kept, dropped_messages) = trim_history(&sanitized, HISTORY_CHAR_LIMIT);
    let history_chars: usize = kept.iter().map(|m| m.content.chars().count()).sum();

    let mut request = Vec::with_capacity(kept.len() + 1);
    request.push(LlmMessage::system(assembled.prompt.clone()));
    request.extend(kept);

    let stats = ContextStats {
        system_chars: assembled.prompt.chars().count(),
        history_chars,
        message_count: request.len(),
        dropped_messages,
        truncated: assembled.truncated || dropped_messages > 0,
    };
    let context = DirectorContext {
        source: source.to_string(),
        built_at: Local::now().to_rfc3339(),
        provider,
        model,
        system_prompt: assembled.prompt,
        sections: assembled.sections,
        messages: request.iter().map(to_context_message).collect(),
        stats,
    };
    (request, context)
}

fn to_context_message(m: &LlmMessage) -> ContextMessage {
    ContextMessage {
        role: m.role.clone(),
        content: (!m.content.is_empty()).then(|| m.content.clone()),
        tool_calls: m.tool_calls.clone(),
        tool_call_id: m.tool_call_id.clone(),
    }
}

// ============================================================
// system 分段
// ============================================================

fn push_section(
    sections: &mut Vec<ContextSection>,
    key: &str,
    title: &str,
    content: String,
    remaining: &mut usize,
    truncated: &mut bool,
) {
    if *remaining == 0 {
        *truncated = true;
        return;
    }
    let len = content.chars().count();
    let body = if len > *remaining {
        *truncated = true;
        let body = truncate_chars(&content, *remaining);
        *remaining = 0;
        body
    } else {
        *remaining -= len;
        content
    };
    sections.push(ContextSection {
        key: key.to_string(),
        title: title.to_string(),
        content: body,
    });
}

fn build_scene(gs: &GameStatus) -> String {
    let mode = match gs.script_status.as_ref() {
        Some(script) => format!("剧本《{}》", script.name),
        None => "自由对话".to_string(),
    };
    let time = gs
        .last_dialog_time
        .as_ref()
        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "未记录".to_string());
    let scene = gs
        .current_scene_id
        .clone()
        .unwrap_or_else(|| "未选择".to_string());
    let speaker = gs
        .current_role_id
        .and_then(|id| role_label(gs, id))
        .unwrap_or_else(|| "无".to_string());

    let mut out = String::from("【当前现场】\n");
    out.push_str(&format!("模式：{mode}\n"));
    out.push_str(&format!("时间：{time}\n"));
    out.push_str(&format!("场景：{scene}\n"));
    out.push_str(&format!("背景：{}\n", present_value(&gs.background)));
    out.push_str(&format!(
        "背景音乐：{}\n",
        present_value(&gs.background_music)
    ));
    out.push_str(&format!("氛围：{}\n", present_value(&gs.background_effect)));
    out.push_str(&format!("天气：{}\n", present_value(&gs.weather_effect)));
    out.push_str(&format!("当前发言者：{speaker}"));
    out
}

fn build_roles(gs: &GameStatus) -> String {
    let mut out = String::from("【在场角色】\n");
    let player = &gs.player;
    let player_name = player.user_name.trim();
    if player_name.is_empty() {
        out.push_str("玩家：未知\n");
    } else {
        let subtitle = player.user_subtitle.trim();
        if subtitle.is_empty() {
            out.push_str(&format!("玩家：{player_name}\n"));
        } else {
            out.push_str(&format!("玩家：{player_name}（{subtitle}）\n"));
        }
    }

    let mut ids: Vec<i32> = gs
        .present_role_ids
        .iter()
        .copied()
        .filter(|id| *id != 0)
        .collect();
    ids.sort_unstable();
    if ids.is_empty() {
        out.push_str("（暂无其他在场角色）");
        return out;
    }

    for id in ids.iter().take(MAX_LISTED_ROLES) {
        let Some(role) = gs.role_manager.get_loaded(*id) else {
            out.push_str(&format!("- #{id}（角色未加载）\n"));
            continue;
        };
        let name = role
            .display_name
            .clone()
            .unwrap_or_else(|| role.settings.ai_name.clone());
        let subtitle = role.settings.ai_subtitle.as_deref().unwrap_or("").trim();
        let info = role.settings.info.as_deref().unwrap_or("").trim();
        out.push_str(&format!("- {name}"));
        if !subtitle.is_empty() {
            out.push_str(&format!("（{subtitle}）"));
        }
        if !gs.onstage_role_ids.contains(id) {
            out.push_str("（不在舞台）");
        }
        if !info.is_empty() {
            out.push_str(&format!("：{}", truncate_chars(info, ROLE_INFO_LIMIT)));
        }
        out.push('\n');
    }
    if ids.len() > MAX_LISTED_ROLES {
        out.push_str(&format!(
            "（另有 {} 位在场角色未列出）",
            ids.len() - MAX_LISTED_ROLES
        ));
    }
    out
}

/// 最近对话段，从最新往回取，装不下就停在更旧的一条之前。
fn build_recent(gs: &GameStatus, window: usize, budget: usize) -> (String, bool) {
    let lines = tail_lines(gs, window);
    let mut kept: Vec<String> = Vec::new();
    let mut used = 0usize;
    let mut truncated = false;
    for line in lines.iter().rev() {
        let text = format_line(gs, line);
        let len = text.chars().count() + 1;
        if used + len > budget {
            truncated = true;
            break;
        }
        used += len;
        kept.push(text);
    }
    kept.reverse();

    if kept.is_empty() {
        return ("【最近对话】\n（还没有对话）".to_string(), truncated);
    }
    (format!("【最近对话】\n{}", kept.join("\n")), truncated)
}

fn format_line(gs: &GameStatus, line: &GameLine) -> String {
    let content = truncate_chars(&line.base.content, LINE_CHAR_LIMIT);
    match line.attribute() {
        LineAttribute::Assistant => {
            let role_id = line.sender_role_id().unwrap_or(0);
            let name = role_label(gs, role_id)
                .or_else(|| line.base.display_name.clone())
                .unwrap_or_else(|| "角色".to_string());
            let emotion = line
                .base
                .predicted_emotion
                .as_deref()
                .or(line.base.original_emotion.as_deref())
                .map(|e| format!("({e})"))
                .unwrap_or_default();
            format!("[{role_id}] {name}{emotion}: {content}")
        },
        LineAttribute::User => {
            let role_id = line.sender_role_id().unwrap_or(0);
            if role_id == 0 {
                format!("[旁白] {content}")
            } else {
                let name = line
                    .base
                    .display_name
                    .clone()
                    .unwrap_or_else(|| gs.player.user_name.clone());
                format!("[{role_id}] {name}: {content}")
            }
        },
        LineAttribute::System => format!("[系统] {content}"),
        LineAttribute::Tool => format!("[工具] {content}"),
    }
}

fn role_label(gs: &GameStatus, role_id: i32) -> Option<String> {
    if role_id == 0 {
        return None;
    }
    let role = gs.role_manager.get_loaded(role_id)?;
    Some(
        role.display_name
            .clone()
            .unwrap_or_else(|| role.settings.ai_name.clone()),
    )
}

fn present_value(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        "无".to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate_chars(s: &str, limit: usize) -> String {
    if s.chars().count() <= limit {
        return s.to_string();
    }
    let mut out: String = s.chars().take(limit.saturating_sub(1)).collect();
    out.push('…');
    out
}

// ============================================================
// 历史清洗与裁剪
// ============================================================

/// 丢弃不成对的工具消息：assistant 的 tool_calls 必须紧跟着全部对应的 tool 结果。
fn sanitize_tool_pairing(messages: &[LlmMessage]) -> Vec<LlmMessage> {
    let mut out = Vec::with_capacity(messages.len());
    let mut i = 0;
    while i < messages.len() {
        let message = &messages[i];
        if message.role == "tool" {
            // 前面没有承接它的 assistant，孤立结果丢弃
            i += 1;
            continue;
        }
        let Some(calls) = &message.tool_calls else {
            out.push(message.clone());
            i += 1;
            continue;
        };

        let mut j = i + 1;
        let mut results = Vec::new();
        while j < messages.len() && messages[j].role == "tool" {
            results.push(messages[j].clone());
            j += 1;
        }
        let paired = !calls.is_empty()
            && calls.iter().all(|call| {
                results
                    .iter()
                    .any(|r| r.tool_call_id.as_deref() == Some(call.id.as_str()))
            });
        if paired {
            out.push(message.clone());
            out.extend(results.into_iter().filter(|r| {
                calls
                    .iter()
                    .any(|call| r.tool_call_id.as_deref() == Some(call.id.as_str()))
            }));
        }
        i = j;
    }
    out
}

/// 历史超出预算时，从最旧的整块开始丢（一块 = 单条消息，或 assistant + 其工具结果）。
fn trim_history(messages: &[LlmMessage], budget: usize) -> (Vec<LlmMessage>, usize) {
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let start = i;
        i += 1;
        if messages[start].tool_calls.is_some() {
            while i < messages.len() && messages[i].role == "tool" {
                i += 1;
            }
        }
        blocks.push((start, i));
    }
    if blocks.is_empty() {
        return (Vec::new(), 0);
    }

    let mut total = 0usize;
    let mut keep_from = blocks.len();
    for (idx, (start, end)) in blocks.iter().enumerate().rev() {
        let len: usize = messages[*start..*end]
            .iter()
            .map(|m| m.content.chars().count())
            .sum();
        // 最新一块无条件保留，否则预算过小时一条都发不出去
        if total + len > budget && keep_from < blocks.len() {
            break;
        }
        total += len;
        keep_from = idx;
    }

    let dropped: usize = blocks
        .iter()
        .take(keep_from)
        .map(|(start, end)| end - start)
        .sum();
    (messages[blocks[keep_from].0..].to_vec(), dropped)
}
