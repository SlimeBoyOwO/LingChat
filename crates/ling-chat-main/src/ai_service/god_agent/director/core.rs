//! 导演 Agent 运行层：存档懒同步、现场装配、流式对话。

use chrono::Local;
use futures_util::StreamExt;
use sea_orm::DatabaseConnection;
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::core::tail_lines;
use crate::ai_service::llm::{LlmChunk, LlmSlot, LlmUsage, slot_snapshot};
use crate::ai_service::skill_agent::db;
use crate::ai_service::skill_agent::events::Usage as AgentUsage;
use crate::ai_service::types::{GameLine, LlmMessage};
use crate::db::entities::agent_message;
use crate::db::entities::line::LineAttribute;

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
}

pub struct DirectorAgent {
    /// 与上帝 Agent 共用同一槽位，热切换 LLM 自动生效。
    pub llm: LlmSlot,
    pub session: tokio::sync::Mutex<DirectorSession>,
}

impl DirectorAgent {
    pub fn new(llm: LlmSlot) -> Self {
        Self {
            llm,
            session: tokio::sync::Mutex::new(DirectorSession::default()),
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

    /// 跑一轮对话。正文边流边推，结束、停止、出错都把已产出的内容落库。
    pub async fn run_chat(
        &self,
        db: DatabaseConnection,
        channel: Channel<DirectorAgentEvent>,
        request: Vec<LlmMessage>,
        turn: TurnContext,
    ) {
        let Some(llm) = slot_snapshot(&self.llm).await else {
            let _ = channel.send(DirectorAgentEvent::Error {
                message: "导演没有可用的对话模型".to_string(),
            });
            self.finish_turn(&db, &turn, String::new(), String::new(), None)
                .await;
            return;
        };

        let mut content = String::new();
        let mut reasoning = String::new();
        let mut usage: Option<LlmUsage> = None;
        let mut error: Option<String> = None;
        let cancel = turn.cancel.clone();

        match llm.complete_stream(&request).await {
            Err(e) => error = Some(format!("导演请求失败：{e}")),
            Ok(mut stream) => {
                loop {
                    tokio::select! {
                        _ = cancel.cancelled() => {
                            let _ = channel.send(DirectorAgentEvent::Status {
                                content: "已停止生成".to_string(),
                            });
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
                                        content.push_str(&c);
                                        let _ = channel.send(DirectorAgentEvent::MessageDelta { content: c });
                                    }
                                    LlmChunk::Reasoning(r) => {
                                        reasoning.push_str(&r);
                                        let _ = channel.send(DirectorAgentEvent::Reasoning { content: r });
                                    }
                                    LlmChunk::StreamEnd { usage: end, .. } => usage = end,
                                    // 本期工具集为空，模型不会返回工具调用；协议先留着
                                    LlmChunk::ToolCalls(_) | LlmChunk::ToolCallProgress { .. } => {}
                                },
                            }
                        }
                    }
                }
            },
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
