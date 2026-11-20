//! 导演 Agent 运行层：存档懒同步、现场装配、流式对话。

use std::collections::HashSet;

use chrono::Local;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use serde_json::{Value, json};
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;

use crate::ai_service::llm::{LlmChunk, LlmClient, LlmSlot, LlmUsage, slot_snapshot};
use crate::ai_service::skill_agent::db;
use crate::ai_service::skill_agent::events::Usage as AgentUsage;
use crate::ai_service::tools::executor::{ToolContext, ToolExecutor};
use crate::ai_service::types::{LlmMessage, ToolCall, ToolDefinition};
use crate::db::entities::agent_message;

use super::context::{AssembledSystem, assemble, compose_request};
use super::draft::{
    DRAFT_INSTRUCTION, DRAFT_MAX_ATTEMPTS, DRAFT_USER_PROMPT, DirectorConfirmResult,
    DirectorDraftView, DirectorModeState, DirectorModeView, draft_event_catalog, parse_draft,
};
use super::tools::{LoopStep, deferred_tool_error, director_registry, plan_next_step};
use super::{DirectorAgentEvent, DirectorContext, DirectorMessage};

pub struct TurnContext {
    pub user_id: i32,
    pub conversation_id: Option<i32>,
    pub cancel: CancellationToken,
    id: u64,
}

impl TurnContext {
    pub fn id(&self) -> u64 {
        self.id
    }
}

/// 会话与当前存档不一致时重新加载。
#[derive(Default)]
pub struct DirectorSession {
    pub save_id: Option<i32>,
    pub conversation_id: Option<i32>,
    pub messages: Vec<DirectorMessage>,
    pub last_context: Option<DirectorContext>,
    pub running: bool,
    pub cancel: Option<CancellationToken>,
    pub task: Option<tauri::async_runtime::JoinHandle<()>>,
    next_turn_id: u64,
    active_turn_id: Option<u64>,
    pub mode: DirectorModeState,
}

pub struct DirectorAgent {
    /// 与上帝 Agent 共用同一槽位，热切换 LLM 自动生效。
    pub llm: LlmSlot,
    pub session: std::sync::Arc<tokio::sync::Mutex<DirectorSession>>,
}

impl DirectorAgent {
    pub fn new(llm: LlmSlot) -> Self {
        Self {
            llm,
            session: std::sync::Arc::new(tokio::sync::Mutex::new(DirectorSession::default())),
        }
    }

    /// 同步会话存档；自由对话转为存档时保留已聊消息。
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
        if session.running {
            return Err("导演正在生成回复，请等待本轮结束后再切换会话".to_string());
        }
        session.last_context = None;
        session.mode.reset_progress();

        match save_id {
            None => {
                session.save_id = None;
                session.conversation_id = None;
                session.messages.clear();
            },
            Some(new_id) => {
                let adopts_memory = session.save_id.is_none() && !session.messages.is_empty();
                let conversation_id = if adopts_memory {
                    Some(db::get_or_create_director_conversation(db, new_id).await?)
                } else {
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

    pub async fn tool_history(&self) -> Vec<DirectorMessage> {
        let session = self.session.lock().await;
        session
            .messages
            .iter()
            .filter(|m| m.role == "tool" || m.tool_calls.is_some())
            .cloned()
            .collect()
    }

    pub async fn set_task(&self, turn_id: u64, task: tauri::async_runtime::JoinHandle<()>) {
        let mut session = self.session.lock().await;
        if session.active_turn_id == Some(turn_id) {
            session.task = Some(task);
        }
    }

    pub async fn take_control(
        &self,
    ) -> (
        Option<CancellationToken>,
        Option<tauri::async_runtime::JoinHandle<()>>,
    ) {
        let mut session = self.session.lock().await;
        (session.cancel.take(), session.task.take())
    }

    /// 达到轮次间隔时异步生成草案，固定剧本或试玩期间不生成。
    pub async fn maybe_generate_draft(&self, app: AppHandle, tools_enabled: bool) {
        if !tools_enabled {
            return;
        }
        let state = app.state::<crate::AppState>();
        let Some(core) = state.god_agent.clone() else {
            return;
        };
        let window = core.config_snapshot().recent_window;
        let (current_seq, save_id, assembled) = {
            let service = state.ai_service.lock().await;
            let gs = service.game_status.lock().await;
            let current_seq = crate::api::game::compute_user_message_seqs(&gs.line_list)
                .into_iter()
                .flatten()
                .max()
                .unwrap_or(0);
            (current_seq, gs.active_save_id, assemble(&gs, window))
        };
        let token = {
            let mut session = self.session.lock().await;
            if session.save_id != save_id {
                return;
            }
            session.mode.begin_generation(current_seq, save_id)
        };
        let Some(token) = token else { return };
        let Some(llm) = slot_snapshot(&core.llm).await else {
            self.finish_draft(token, Err("导演没有可用的对话模型".to_string()))
                .await;
            return;
        };
        let session = self.session.clone();
        tauri::async_runtime::spawn(async move {
            let result = generate_draft(llm.as_ref(), assembled).await;
            session.lock().await.mode.finish_generation(token, result);
        });
    }

    pub async fn mode_view(&self) -> DirectorModeView {
        self.session.lock().await.mode.mode_view()
    }

    pub async fn draft_view(&self) -> DirectorDraftView {
        self.session.lock().await.mode.draft_view()
    }

    pub async fn set_mode(
        &self,
        enabled: bool,
        interval: Option<usize>,
    ) -> Result<DirectorModeView, String> {
        let mut session = self.session.lock().await;
        session.mode.set_mode(enabled, interval)?;
        Ok(session.mode.mode_view())
    }

    pub async fn discard_draft(&self) -> Result<DirectorDraftView, String> {
        let mut session = self.session.lock().await;
        session.mode.discard()?;
        Ok(session.mode.draft_view())
    }

    pub async fn confirm_draft(&self, app: AppHandle) -> Result<DirectorConfirmResult, String> {
        let state = app.state::<crate::AppState>();
        let save_id = {
            let service = state.ai_service.lock().await;
            service.game_status.lock().await.active_save_id
        };
        let (token, events) = {
            let mut session = self.session.lock().await;
            session
                .mode
                .begin_confirm(save_id)
                .ok_or_else(|| "当前没有待确认的草案，或存档已切换".to_string())?
        };
        let mut applied = 0usize;
        let mut failure = None;
        for (index, item) in events.iter().enumerate() {
            if current_save_id(&state).await != save_id {
                failure = Some("存档已切换，剩余草案事件已停止投递".to_string());
                break;
            }
            let type_key = item["event"].as_str().unwrap_or_default();
            let params = item.get("params").cloned().unwrap_or_else(|| json!({}));
            if let Err(error) =
                super::event_tools::execute_director_event(&app, type_key, params).await
            {
                failure = Some(format!("第 {} 个事件 {type_key}：{error}", index + 1));
                break;
            }
            applied += 1;
        }
        if current_save_id(&state).await != save_id {
            failure.get_or_insert_with(|| "存档已切换，草案不能在新存档中完成确认".to_string());
        }
        let current_seq = current_user_seq(&state).await;
        let mut session = self.session.lock().await;
        if !session
            .mode
            .finish_confirm(token, current_seq, applied, failure.clone())
        {
            return Err("草案状态已变化，本次确认结果未写回".to_string());
        }
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
        session.next_turn_id = session.next_turn_id.wrapping_add(1);
        let id = session.next_turn_id;
        session.active_turn_id = Some(id);

        Ok(TurnContext {
            user_id,
            conversation_id,
            cancel,
            id,
        })
    }

    /// 保存本轮实际请求的上下文快照。
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

    /// 重新装配快照，但不覆盖上次实际请求。
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

    /// 运行一轮对话，停止或出错时仍保存已产出的内容。
    /// tools_enabled 为 false 时不提供工具，并拒绝残留调用。
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
            let _ = self
                .finish_turn(&db, &turn, String::new(), String::new(), None)
                .await;
            return;
        };

        let registry = director_registry();
        let streaming_tools = llm.supports_streaming_tools();
        let definitions: Vec<ToolDefinition> = if tools_enabled {
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
            let use_tools = !final_synthesis && !definitions.is_empty();
            let response = receive_response(
                llm.as_ref(),
                &messages,
                &definitions,
                use_tools,
                streaming_tools,
                &channel,
                &cancel,
            )
            .await;
            let (round_content, round_reasoning, round_calls) = match response {
                Ok(output) => {
                    if output.usage.is_some() {
                        usage = output.usage;
                    }
                    (output.content, output.reasoning, output.calls)
                },
                Err(failure) => {
                    if failure.cancelled {
                        stopped = true;
                    } else {
                        error = Some(failure.message);
                    }
                    (failure.output.content, failure.output.reasoning, Vec::new())
                },
            };

            if round_calls.is_empty() || final_synthesis {
                content.push_str(&round_content);
                reasoning.push_str(&round_reasoning);
            }
            if error.is_some() || stopped {
                break;
            }

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

            let mut assistant_msg = LlmMessage::tool(round_calls.clone());
            assistant_msg.content = round_content.clone();
            if let Err(message) = self
                .append_loop_message(&db, &turn, &assistant_msg, nonempty(&round_reasoning))
                .await
            {
                error = Some(message);
                break;
            }
            messages.push(assistant_msg);

            if !tools_enabled {
                let reason = "当前正在运行固定剧本，导演工具已关闭；请结束剧本后再安排剧情。";
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
                    if let Err(message) =
                        self.append_loop_message(&db, &turn, &tool_msg, None).await
                    {
                        error = Some(message);
                        break;
                    }
                    messages.push(tool_msg);
                }
                if error.is_some() {
                    break;
                }
                final_synthesis = true;
                continue;
            }

            // 取消后仍保留配对结果，避免消息历史失配。
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
                } else if let Some(output) = deferred_tool_error(index) {
                    output
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
                if let Err(message) = self.append_loop_message(&db, &turn, &tool_msg, None).await {
                    error = Some(message);
                    break;
                }
                messages.push(tool_msg);
            }
            if stopped || error.is_some() {
                if stopped {
                    let _ = channel.send(DirectorAgentEvent::Status {
                        content: "已停止生成".to_string(),
                    });
                }
                break;
            }

            match plan_next_step(round, true) {
                LoopStep::Finish => break,
                LoopStep::ContinueWithTools => round += 1,
                LoopStep::FinalSynthesis => {
                    final_synthesis = true;
                    round += 1;
                },
            }
        }

        let message_id = match self
            .finish_turn(&db, &turn, content.clone(), reasoning, usage)
            .await
        {
            Ok(id) => id,
            Err(message) => {
                error = Some(message);
                0
            },
        };
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

    /// 工具调用与结果按原顺序写入本轮会话。
    async fn append_loop_message(
        &self,
        db: &DatabaseConnection,
        turn: &TurnContext,
        message: &LlmMessage,
        reasoning: Option<&str>,
    ) -> Result<(), String> {
        let mut id = 0;
        if let Some(conversation_id) = turn.conversation_id {
            match db::append_director_message(db, conversation_id, message, reasoning, None).await {
                Ok(new_id) => id = new_id,
                Err(e) => return Err(format!("导演工具消息落库失败: {e}")),
            }
        }

        let mut session = self.session.lock().await;
        if session.active_turn_id == Some(turn.id)
            && session.conversation_id == turn.conversation_id
        {
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
        Ok(())
    }

    async fn finish_turn(
        &self,
        db: &DatabaseConnection,
        turn: &TurnContext,
        content: String,
        reasoning: String,
        usage: Option<LlmUsage>,
    ) -> Result<i32, String> {
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
                    Err(e) => return Err(format!("导演回复落库失败: {e}")),
                }
            }
        }

        let mut session = self.session.lock().await;
        if session.active_turn_id != Some(turn.id) {
            return Ok(message_id);
        }
        session.running = false;
        session.cancel = None;
        session.active_turn_id = None;
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
        Ok(message_id)
    }
}

#[derive(Default)]
struct ModelOutput {
    content: String,
    reasoning: String,
    calls: Vec<ToolCall>,
    usage: Option<LlmUsage>,
}

struct ResponseFailure {
    message: String,
    cancelled: bool,
    output: ModelOutput,
}

/// 读取一轮流式或非流式模型响应，停止等待时也返回已产生的正文。
async fn receive_response(
    llm: &LlmClient,
    messages: &[LlmMessage],
    definitions: &[ToolDefinition],
    use_tools: bool,
    streaming_tools: bool,
    channel: &Channel<DirectorAgentEvent>,
    cancel: &CancellationToken,
) -> Result<ModelOutput, ResponseFailure> {
    if use_tools && !streaming_tools {
        let response = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(cancelled(channel, ModelOutput::default()));
            },
            result = llm.complete_with_tools(messages, definitions, Some("auto")) => result,
        };
        let response = response.map_err(|e| ResponseFailure {
            message: format!("导演请求失败：{e}"),
            cancelled: false,
            output: ModelOutput::default(),
        })?;
        let content = response.content.unwrap_or_default();
        if !content.is_empty() {
            let _ = channel.send(DirectorAgentEvent::MessageDelta {
                content: content.clone(),
            });
        }
        return Ok(ModelOutput {
            content,
            reasoning: String::new(),
            calls: response.tool_calls.unwrap_or_default(),
            usage: response.usage,
        });
    }

    let response = if use_tools {
        llm.complete_stream_with_tools(messages, definitions, Some("auto"))
            .await
    } else {
        llm.complete_stream(messages).await
    };
    let mut stream = response.map_err(|e| ResponseFailure {
        message: format!("导演请求失败：{e}"),
        cancelled: false,
        output: ModelOutput::default(),
    })?;
    let mut output = ModelOutput {
        content: String::new(),
        reasoning: String::new(),
        calls: Vec::new(),
        usage: None,
    };
    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => {
                return Err(cancelled(channel, output));
            },
            chunk = stream.next() => chunk,
        };
        match chunk {
            None => break,
            Some(Err(e)) => {
                return Err(ResponseFailure {
                    message: e.to_string(),
                    cancelled: false,
                    output,
                });
            },
            Some(Ok(LlmChunk::Content(text))) => {
                output.content.push_str(&text);
                let _ = channel.send(DirectorAgentEvent::MessageDelta { content: text });
            },
            Some(Ok(LlmChunk::Reasoning(text))) => {
                output.reasoning.push_str(&text);
                let _ = channel.send(DirectorAgentEvent::Reasoning { content: text });
            },
            Some(Ok(LlmChunk::ToolCalls(calls))) => output.calls.extend(calls),
            Some(Ok(LlmChunk::StreamEnd { usage, .. })) => {
                if usage.is_some() {
                    output.usage = usage;
                }
            },
            Some(Ok(LlmChunk::ToolCallProgress { .. })) => {},
        }
    }
    Ok(output)
}

fn cancelled(channel: &Channel<DirectorAgentEvent>, output: ModelOutput) -> ResponseFailure {
    let _ = channel.send(DirectorAgentEvent::Status {
        content: "已停止生成".to_string(),
    });
    ResponseFailure {
        message: String::new(),
        cancelled: true,
        output,
    }
}

async fn current_user_seq(state: &crate::AppState) -> u32 {
    let service = state.ai_service.lock().await;
    let gs = service.game_status.lock().await;
    crate::api::game::compute_user_message_seqs(&gs.line_list)
        .into_iter()
        .flatten()
        .max()
        .unwrap_or(0)
}

async fn current_save_id(state: &crate::AppState) -> Option<i32> {
    let service = state.ai_service.lock().await;
    service.game_status.lock().await.active_save_id
}

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

/// 基于现场生成并校验草案，失败时重试。
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
