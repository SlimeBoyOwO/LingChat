use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{Mutex, oneshot};

use crate::AppState;
use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmClient};
use crate::ai_service::message_system::generator::GeneratorSource;
use crate::ai_service::message_system::producer::{PresentationChunk, PresentationStream};
use crate::ai_service::message_system::responses::event_names;
use crate::ai_service::types::{LlmMessage, ToolDefinition};
use crate::utils::prompt::{ReplyLengthMode, scan_reply_shape};

use super::executor::{ToolContext, ToolExecutor};
use super::registry::ToolRegistry;
use super::tool_rag::{apply_dynamic_search_description, effective_allowed};

/// 工具结果会写入角色长期记忆；限制单轮会话累计量，避免大文件/命令输出永久撑大上下文。
const MAX_PERSISTED_TOOL_RESULT_CHARS: usize = 32_000;
const MAX_PERSISTED_SINGLE_TOOL_RESULT_CHARS: usize = 12_000;
const TOOL_RESULT_TRUNCATION_MARKER: &str = "\n...[工具结果过长，长期记忆已截断]";
// 工具结果返回后的续写引导由 ReplyLengthMode::tool_continuation_text 生成，再经
// attach_line_budget 写进最后一条工具结果。不能在对话中间追加 system 消息：部分
// 提供商会把它当成新的人设提示，导致人设漂移。
/// 收尾轮只返回工具调用、没有任何正文时的补救提示：明确调用未执行、已无工具可用，
/// 要求直接输出总结正文，避免整条回复落空触发前端报错。
const FINAL_SYNTHESIS_RETRY_PROMPT: &str = "你刚才尝试调用的工具未被执行：当前已没有可用工具。请不要再尝试调用工具，直接基于已完成的工具结果，用正文给出最终答复；如仍有未完成事项，请明确说明。";
/// 工具轮数用满、收掉工具定义后的收尾提示：还要报一次本回合的台词额度。
fn final_synthesis_text(mode: ReplyLengthMode, spoken: usize) -> String {
    let mut text = String::from(
        "工具调用已达到本轮上限。请停止调用工具，基于已有工具结果直接给出最终答复；如仍有未完成事项，请明确说明。",
    );
    if let Some(budget) = mode.tool_turn_budget() {
        let left = budget.saturating_sub(spoken);
        if left == 0 {
            text.push_str("本回合的台词额度已经用完，不必再输出台词。");
        } else {
            text.push_str(&format!(
                "本回合已经说了 {spoken} 句台词，这一条最多再补 {left} 句。"
            ));
        }
    }
    text
}

/// TODO: 钦灵：这部分代码我会重点审查，今天困了（
/// 把一段工具循环指令写进最后一条工具结果。
fn attach_tool_note(messages: &mut [LlmMessage], key: &str, note: serde_json::Value) {
    let Some(last) = messages
        .iter_mut()
        .rev()
        .find(|message| message.role == "tool")
    else {
        return;
    };
    if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&last.content) {
        if let Some(object) = value.as_object_mut() {
            object.insert(key.to_string(), note);
            last.content = value.to_string();
            return;
        }
    }
    let text = note
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| note.to_string());
    last.content = format!("{}\n\n（{text}）", last.content);
}

/// 把本回合的台词额度塞进最后一条工具结果，只改这一轮请求的副本。
fn attach_line_budget(messages: &mut [LlmMessage], mode: ReplyLengthMode, spoken: usize) {
    let Some(budget) = mode.tool_turn_budget() else {
        // 自由档没有额度，但仍然要告诉模型可以不说话
        attach_tool_note(
            messages,
            "tool_continuation",
            serde_json::Value::String(mode.tool_continuation_text(spoken)),
        );
        return;
    };
    let left = budget.saturating_sub(spoken);
    let note = format!(
        "{}这是内部限制，不要向玩家复述。",
        mode.tool_continuation_text(spoken)
    );
    attach_tool_note(
        messages,
        "line_budget",
        serde_json::json!({
            "spoken": spoken,
            "budget": budget,
            "left": left,
            "note": note,
        }),
    );
}

/// 工具执行前等待"前导台词已经 emit"的上限。
///
/// 这段等待包含 consumer 侧的一次翻译 LLM 往返与 TTS 合成，因此宁宽勿窄；
/// 超时后**放行工具**（fail-open）——顺序不完美也好过让工具链路被富化卡死。
const FENCE_ACK_TIMEOUT: Duration = Duration::from_secs(30);

/// 工具消息收集槽：流消费过程中由闭环填充，按「一轮工具调用」分组。
/// 消费完毕后调用方逐轮取走，把每轮的 assistant(tool_calls) 与 tool 结果回填到对应前导行之后。
pub type ToolMessageSink = Arc<Mutex<Vec<Vec<LlmMessage>>>>;

pub struct ToolLoopResult {
    pub stream: PresentationStream,
    pub tool_messages: ToolMessageSink,
    /// 工具闭环是否真的执行过工具。生产者据此只在工具场景暂存最后一段，
    /// 让重复的收尾段可以被丢弃，同时仍把最后一条有效回复标为完成。
    pub tool_calls_seen: Arc<AtomicBool>,
}

/// 保证工具参数进度在正常结束、流错误和上游取消时都会被清理。
///
/// `async_stream` 在消费方提前丢弃流时不会继续执行循环后的语句，因此清理必须
/// 绑定到 RAII guard，不能只依赖正常路径中的显式事件。
struct ToolCallProgressEndGuard {
    app: AppHandle,
}

impl ToolCallProgressEndGuard {
    fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl Drop for ToolCallProgressEndGuard {
    fn drop(&mut self) {
        emit_tool_call_progress_end_event(&self.app);
    }
}

/// 以流式请求执行普通聊天的工具闭环。
///
/// 仅支持原生流式 tools 的 provider 会携带工具定义请求。工具调用必须等到本轮
/// 流结束后才会执行，以确保参数已经由 provider 合并完整。其他 provider 保持
/// 单次普通流式请求，避免退回非流式预检。
///
/// 闭环是**惰性**的：决策轮次在返回的流内部驱动，内容/思考块实时透传给下游
/// （前端思考字数、句子切分保持流式体验）；工具消息累积进 `ToolMessageSink`，
/// 待流消费完毕后由调用方读取并回填台词。
// 入参就是本轮工具闭环需要的全部上下文，拆结构体只会多一层转手，这里接受 8 个参数。
#[allow(clippy::too_many_arguments)]
pub async fn stream_with_tool_loop(
    llm: &Arc<LlmClient>,
    registry: &Arc<ToolRegistry>,
    messages: Vec<LlmMessage>,
    source: GeneratorSource,
    role_name: Option<String>,
    app: &AppHandle,
    recommended: &[String],
    reply_mode: ReplyLengthMode,
) -> Result<ToolLoopResult> {
    // 每次回复开始时读取一次设置，避免同一轮执行期间修改配置导致边界跳变。
    let (settings, tool_rag) = {
        let state = app.state::<AppState>();
        (state.tool_settings.get(), state.tool_rag.clone())
    };
    let max_tool_rounds = settings.tool_round_limit();
    // 模型缺失 / onnxruntime 不可用时 is_enabled 为 false，自动回退到全量工具下发。
    let rag_on = settings.tool_rag && tool_rag.is_enabled();
    if settings.tool_rag && !tool_rag.is_enabled() {
        tracing::info!("Tool RAG 已开启但嵌入引擎不可用，本次回退为全量工具下发");
    }

    let initial_base_allowed = registry.allowed_tools(source, role_name.as_deref());
    let initial_allowed = effective_allowed(
        &initial_base_allowed,
        &tool_rag,
        role_name.as_deref(),
        rag_on,
    );
    let initial_definitions = registry.definitions_for_allowed(&initial_allowed);
    if initial_definitions.is_empty() || !llm.supports_streaming_tools() {
        if !initial_definitions.is_empty() {
            tracing::info!("当前 LLM Provider 不支持原生流式工具调用，跳过普通聊天工具闭环");
        }
        return Ok(ToolLoopResult {
            stream: presentation_stream(llm.complete_stream(&messages).await?),
            tool_messages: Arc::new(Mutex::new(Vec::new())),
            tool_calls_seen: Arc::new(AtomicBool::new(false)),
        });
    }

    let tool_messages: ToolMessageSink = Arc::new(Mutex::new(Vec::new()));
    let sink = tool_messages.clone();
    let tool_calls_seen = Arc::new(AtomicBool::new(false));
    let tool_calls_seen_in_stream = tool_calls_seen.clone();

    // 流需要 'static：闭环状态全部改为持有所有权（Arc/克隆）
    let llm = llm.clone();
    let registry = registry.clone();
    let app = app.clone();
    let tool_rag = tool_rag.clone();
    let recommended = recommended.to_vec();

    let stream = async_stream::try_stream! {
        let executor = ToolExecutor::new(&registry);
        let mut messages = messages;
        // 切换到一个原本不在场的角色时，它的历史记忆不会包含触发切换的用户消息；
        // 保留这一条，重建上下文后补回，避免新角色只看到孤立的 tool_result。
        let active_user_message = messages
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .cloned();
        let mut active_role_name = role_name;
        let mut persisted_tool_result_chars = 0usize;
        // 这一回合已经说出去的台词句数：台词额度按整回合算，不按单轮算。
        // 每轮各自守「1~2 句」的话，连着几个工具下来就是十几句，而且大半在复述旧事。
        let mut spoken_lines = 0usize;

        // 收尾轮（无工具定义）可能仍返回工具调用且不给正文，多留一轮补救重试。
        let mut synthesis_retried = false;
        for round in 0..=(max_tool_rounds + 1) {
            tracing::info!(round = round + 1, max_tool_rounds, "开始流式聊天工具决策");
            let final_synthesis = round >= max_tool_rounds;
            if round == max_tool_rounds {
                // 达到用户配置的工具轮数后保留一次不带工具定义的收尾生成，避免直接报错并
                // 丢掉已经完成的工具结果。收尾要求写进工具结果，不追加 system 消息。
                attach_tool_note(
                    &mut messages,
                    "final_synthesis",
                    serde_json::Value::String(final_synthesis_text(reply_mode, spoken_lines)),
                );
            }
            // 角色可以在上一轮工具执行中发生变化。每轮按当前角色重新计算权限，
            // 防止切换后沿用旧角色的工具授权。Tool RAG 下还要重算激活集：第 N 轮的
            // tool_search 把命中的工具写进缓存，第 N+1 轮重算时它们才作为定义下发，
            // 「先检索再调用」因此成立。
            let base_allowed = registry.allowed_tools(source, active_role_name.as_deref());
            let allowed = activate_recommended(
                effective_allowed(
                    &base_allowed,
                    &tool_rag,
                    active_role_name.as_deref(),
                    rag_on,
                ),
                &base_allowed,
                &recommended,
            );
            let definitions = if final_synthesis {
                Vec::new()
            } else {
                let mut definitions = registry.definitions_for_allowed(&allowed);
                if rag_on {
                    // tool_search 的描述要反映「本轮到底能检索到哪些工具」。
                    // 只给模型一个「可以检索」的入口、不给任何能力线索，它不会想到去检索，
                    // 只会直接回答做不到。
                    let available = registry.definitions_for_allowed(&base_allowed);
                    apply_dynamic_search_description(&mut definitions, &available);
                    if round == 0 && tracing::enabled!(tracing::Level::DEBUG) {
                        // 只在真的开了 debug 时算体积，避免每轮为日志做无谓的序列化。
                        let payload_chars = |list: &[ToolDefinition]| -> usize {
                            list.iter()
                                .map(|def| {
                                    def.function.name.len()
                                        + def.function.description.len()
                                        + def.function.parameters.to_string().len()
                                })
                                .sum()
                        };
                        tracing::debug!(
                            sent = definitions.len(),
                            searchable = available.len(),
                            sent_chars = payload_chars(&definitions),
                            full_chars = payload_chars(&available),
                            "Tool RAG 工具集已收窄"
                        );
                    }
                }
                definitions
            };
            let mut response_stream = if definitions.is_empty() {
                llm.complete_stream(&messages).await?
            } else {
                llm.complete_stream_with_tools(&messages, &definitions, Some("auto"))
                    .await?
            };
            let progress_end_guard = ToolCallProgressEndGuard::new(app.clone());
            let mut tool_calls = Vec::new();
            let mut round_text = String::new();

            while let Some(chunk) = response_stream.next().await {
                match chunk? {
                    LlmChunk::ToolCalls(calls) => tool_calls.extend(calls),
                    LlmChunk::ToolCallProgress { name, chars } => {
                        // 参数生成进度直接转为前端事件，不进正文流
                        emit_tool_call_progress_event(&app, &name, chars);
                    }
                    LlmChunk::Content(text) => {
                        round_text.push_str(&text);
                        // 实时透传，保持下游流式体验
                        yield PresentationChunk::Chunk(LlmChunk::Content(text));
                    }
                    // Thinking 等其它 chunk 也一起实时透传
                    other => {
                        yield PresentationChunk::Chunk(other);
                    }
                }
            }

            // 正常路径立即结束进度；流错误或消费方提前取消时由 Drop 自动兜底。
            drop(progress_end_guard);
            // 这一轮说了几句：与纠偏同一套分句规则，够用来记整回合的账
            spoken_lines += scan_reply_shape(&round_text).len();

            if tool_calls.is_empty() {
                // 本轮没有工具调用，工具闭环结束
                return;
            }
            if final_synthesis {
                tracing::warn!(
                    count = tool_calls.len(),
                    "无工具定义的最终收尾仍返回工具调用，已忽略"
                );
                // 收尾轮只返回工具调用、正文为空时，整条回复会落空并触发前端报错；
                // 明确告知调用未执行、已无工具可用，补救重试一次让模型直接输出正文。
                if round_text.trim().is_empty() && !synthesis_retried {
                    synthesis_retried = true;
                    tracing::info!("收尾轮无正文，追加补救提示后重试一次");
                    attach_tool_note(
                        &mut messages,
                        "final_synthesis",
                        serde_json::Value::String(FINAL_SYNTHESIS_RETRY_PROMPT.to_string()),
                    );
                    continue;
                }
                return;
            }

            tool_calls_seen_in_stream.store(true, Ordering::Release);
            let calls = tool_calls;
            if let Some(budget) = reply_mode.tool_turn_budget() {
                tracing::info!(
                    spoken = spoken_lines,
                    budget,
                    left = budget.saturating_sub(spoken_lines),
                    "工具回合台词额度"
                );
            }

            let mut ids = HashSet::new();
            for call in &calls {
                if call.id.trim().is_empty() {
                    Err(anyhow!("Provider 返回了空工具调用 ID"))?;
                }
                if !ids.insert(call.id.clone()) {
                    Err(anyhow!("Provider 返回了重复工具调用 ID: {}", call.id))?;
                }
            }

            // ── 呈现栅栏：工具执行前，先等本轮已透传的正文真正 emit 出去 ──
            // 工具事件是裸 `app.emit`，而 `ai:reply` 要过 consumer 的翻译/TTS 才由
            // publisher 发出；没有这个同步点，工具卡片必然抢在前导台词之前出现。
            // 栅栏自身占一个句序号，publisher 按序推进到它时才回 ack（见
            // `producer::PresentationChunk::BeforeTools`）。
            let (fence_tx, fence_rx) = oneshot::channel();
            yield PresentationChunk::BeforeTools(fence_tx);
            let fence_started = Instant::now();
            match tokio::time::timeout(FENCE_ACK_TIMEOUT, fence_rx).await {
                Ok(Ok(reply_expected)) => {
                    tracing::info!(
                        waited_ms = fence_started.elapsed().as_millis() as u64,
                        reply_expected,
                        "工具执行前的呈现栅栏已放行"
                    );
                },
                Ok(Err(_)) => {
                    // ack 被丢弃：emit 失败 / producer 报错 / 整轮取消。此时执行有副作用
                    // 的工具不可接受，且必须在此刻返回——早于下面组装 assistant_message，
                    // 否则历史里会留下"有 tool_calls 却无 tool 结果"的坏配对。
                    tracing::warn!("工具执行前的呈现栅栏被丢弃，本轮工具调用取消");
                    return;
                },
                Err(_) => {
                    // 等太久（通常卡在翻译/TTS）：放行工具，避免工具链路被富化阻塞。
                    // 此时顺序可能不完美，但整轮不会挂死。
                    tracing::warn!(
                        timeout_ms = FENCE_ACK_TIMEOUT.as_millis() as u64,
                        "等待前导台词呈现超时，继续执行工具（顺序可能不完美）"
                    );
                },
            };

            // 本轮的内容文本 + tool_calls 一起存入助理消息
            let assistant_message = LlmMessage {
                role: "assistant".to_string(),
                content: round_text,
                tool_calls: Some(calls.clone()),
                tool_call_id: None,
                image_data_url: None,
            };
            let mut round_messages = vec![assistant_message];
            // 工具附带的图片：只进本轮请求，顺着工具结果后面另起一条用户消息（tool
            // 角色带不了图）。不进 sink，也就不进长期记忆。
            let mut pending_images: Vec<LlmMessage> = Vec::new();
            let context = ToolContext::new(allowed)
                .with_base_allowed(base_allowed)
                .with_role_name(active_role_name.clone())
                .with_app(app.clone());
            let mut character_switched = false;
            for call in calls {
                tracing::info!(tool = call.function.name, call_id = call.id, "执行聊天工具");

                if context.base_allowed.contains(&call.function.name)
                    && !context.allowed_tools.contains(&call.function.name)
                {
                    tracing::info!(
                        tool = %call.function.name,
                        "本轮没有下发这个工具（模型凭记忆直接调用），已重新激活"
                    );
                    tool_rag.touch(
                        active_role_name.as_deref(),
                        std::slice::from_ref(&call.function.name),
                    );
                }
                emit_tool_activity_event(
                    &app,
                    &call.id,
                    &call.function.name,
                    &call.function.arguments,
                    "started",
                    None,
                );
                let result = executor
                    .execute(&call.function.name, &call.function.arguments, &context)
                    .await;
                let succeeded = emit_tool_call_event(
                    &app,
                    &call.id,
                    &call.function.name,
                    &call.function.arguments,
                    &result,
                );
                emit_tool_activity_event(
                    &app,
                    &call.id,
                    &call.function.name,
                    &call.function.arguments,
                    "finished",
                    Some(succeeded),
                );
                if call.function.name == "character_switch" && succeeded {
                    character_switched = true;
                }
                let (content, image) = split_tool_image(&result);
                let tool_result = LlmMessage::tool_result(call.id, content);
                round_messages.push(tool_result);
                if let Some((hint, data_url)) = image {
                    pending_images.push(LlmMessage::user_with_image(hint, data_url));
                }
            }

            let mut live_round_messages = round_messages.clone();
            attach_line_budget(&mut live_round_messages, reply_mode, spoken_lines);

            if character_switched {
                // character_switch 已为目标角色注入 SYSTEM 并刷新记忆。这里必须切换
                // LLM 上下文本身，否则同一请求的最终回复仍会沿用旧角色人设。
                let (refreshed, refreshed_name) = current_role_context(&app).await?;
                messages = rebase_after_character_switch(
                    refreshed,
                    active_user_message.as_ref(),
                    &live_round_messages,
                );
                messages.extend(pending_images);
                active_role_name = refreshed_name;
            } else {
                messages.extend(live_round_messages);
                messages.extend(pending_images);
            }

            let persisted_messages = bounded_tool_history(
                &round_messages,
                &mut persisted_tool_result_chars,
            );
            sink.lock().await.push(persisted_messages);
        }
    };

    Ok(ToolLoopResult {
        stream: Box::pin(stream),
        tool_messages,
        tool_calls_seen,
    })
}

/// 单张工具附带图片的上限（data URL 字符数，约 6MB 原图）。超了只记日志、不带图。
const MAX_TOOL_IMAGE_CHARS: usize = 8 * 1024 * 1024;
/// 工具给了图却没给说明时的兜底提示。
const TOOL_IMAGE_HINT: &str = "（这次工具调用附了一张图片，请一起看。）";

/// 把工具结果里的图片摘出来。
///
/// 约定：`{"image": {"data_url": "data:image/…", "hint": "说明"}}`。返回
/// `(落进长期记忆的文本, 要附给模型的那一条)`。图必须从文本里摘掉——base64 写进记忆
/// 既白占上下文，下一轮还会被重新发一遍；它只属于本轮请求。
fn split_tool_image(result: &str) -> (String, Option<(String, String)>) {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(result) else {
        return (result.to_string(), None);
    };
    let Some(object) = value.as_object_mut() else {
        return (result.to_string(), None);
    };
    let Some(serde_json::Value::Object(image)) = object.remove("image") else {
        return (result.to_string(), None);
    };
    let Some(data_url) = image
        .get("data_url")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
    else {
        return (value.to_string(), None);
    };
    if data_url.len() > MAX_TOOL_IMAGE_CHARS {
        tracing::warn!(chars = data_url.len(), "工具附带的图片过大，已丢弃");
        return (value.to_string(), None);
    }
    let hint = image
        .get("hint")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or(TOOL_IMAGE_HINT)
        .to_string();
    (value.to_string(), Some((hint, data_url)))
}

/// 把上帝 Agent 推荐的工具并进本轮工具集。
///
/// Tool RAG 下推荐的理由才成立：它们随本请求作为定义下发，模型不用先 `tool_search`
/// 就能直接调用；权限集之外的名字一律丢掉（推荐不构成授权）。RAG 关闭时全量下发，
/// 这一步是空操作。
fn activate_recommended(
    mut allowed: HashSet<String>,
    base_allowed: &HashSet<String>,
    recommended: &[String],
) -> HashSet<String> {
    for name in recommended {
        if base_allowed.contains(name) {
            allowed.insert(name.clone());
        }
    }
    allowed
}

/// 当前请求继续使用完整工具结果完成推理；只对写入角色长期记忆的副本做有界裁剪。
/// 保留每条 tool_call_id 与对应消息，避免破坏 provider 的工具调用配对。
fn bounded_tool_history(messages: &[LlmMessage], persisted_chars: &mut usize) -> Vec<LlmMessage> {
    messages
        .iter()
        .cloned()
        .map(|mut message| {
            if message.role != "tool" {
                return message;
            }

            let original_chars = message.content.chars().count();
            let remaining = MAX_PERSISTED_TOOL_RESULT_CHARS.saturating_sub(*persisted_chars);
            let limit = remaining.min(MAX_PERSISTED_SINGLE_TOOL_RESULT_CHARS);
            if original_chars > limit {
                let marker_chars = TOOL_RESULT_TRUNCATION_MARKER.chars().count();
                if limit >= marker_chars {
                    let preview_chars = limit - marker_chars;
                    message.content = message.content.chars().take(preview_chars).collect();
                    message.content.push_str(TOOL_RESULT_TRUNCATION_MARKER);
                } else {
                    message.content = TOOL_RESULT_TRUNCATION_MARKER.chars().take(limit).collect();
                }
            }
            *persisted_chars = persisted_chars.saturating_add(message.content.chars().count());
            message
        })
        .collect()
}

/// 读取 character_switch 完成后的角色记忆快照与权限名称。
async fn current_role_context(app: &AppHandle) -> Result<(Vec<LlmMessage>, Option<String>)> {
    let state = app.state::<AppState>();
    let game_status = {
        let service = state.ai_service.lock().await;
        service.game_status.clone()
    };
    let gs = game_status.lock().await;
    let role_id = gs
        .current_role_id
        .ok_or_else(|| anyhow!("角色切换后没有 current_role_id"))?;
    let role = gs
        .role_manager
        .get_loaded(role_id)
        .ok_or_else(|| anyhow!("角色切换后角色 {role_id} 未加载"))?;
    Ok((role.memory.clone(), role.display_name.clone()))
}

fn rebase_after_character_switch(
    mut refreshed: Vec<LlmMessage>,
    active_user: Option<&LlmMessage>,
    round_messages: &[LlmMessage],
) -> Vec<LlmMessage> {
    if let Some(user) = active_user {
        let already_present = refreshed
            .iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content == user.content)
            .unwrap_or(false);
        if !already_present {
            refreshed.push(user.clone());
        }
    }
    refreshed.extend_from_slice(round_messages);
    refreshed
}

/// 向前端广播工具开始/结束事件，call_id 用于正确处理连续或并发调用。
pub(crate) fn emit_tool_activity_event(
    app: &AppHandle,
    call_id: &str,
    tool: &str,
    arguments: &str,
    phase: &str,
    ok: Option<bool>,
) {
    let arguments_detail: String = arguments.chars().take(1000).collect();
    let payload = serde_json::json!({
        "call_id": call_id,
        "tool": tool,
        "phase": phase,
        "ok": ok,
        "arguments": arguments_detail,
    });
    if let Err(error) = app.emit(event_names::AI_TOOL_ACTIVITY, &payload) {
        tracing::warn!("emit ai:tool_activity 失败: {error}");
    }
}

/// 工具调用参数的流式生成进度：驱动前端顶栏「正在写入… N 字」实时提示。
pub(crate) fn emit_tool_call_progress_event(app: &AppHandle, tool: &str, chars: usize) {
    let payload = serde_json::json!({
        "tool": tool,
        "chars": chars,
    });
    if let Err(error) = app.emit(event_names::AI_TOOL_CALL_PROGRESS, &payload) {
        tracing::warn!("emit ai:tool_call_progress 失败: {error}");
    }
}

/// 一轮 LLM 流结束：通知前端清除参数生成进度提示（无载荷）。
pub(crate) fn emit_tool_call_progress_end_event(app: &AppHandle) {
    if let Err(error) = app.emit(event_names::AI_TOOL_CALL_PROGRESS_END, ()) {
        tracing::warn!("emit ai:tool_call_progress_end 失败: {error}");
    }
}

/// 工具执行后向前端广播 `ai:tool_call` 事件（用于调用提示/通知）。
/// 返回与事件一致的成功状态，供生命周期提示和角色切换判断复用。
pub(crate) fn emit_tool_call_event(
    app: &AppHandle,
    call_id: &str,
    tool: &str,
    arguments: &str,
    result: &str,
) -> bool {
    // executor 的可恢复错误统一编码为 {"ok": false, "error": {...}}；
    // 成功结果带 "ok": true。工具自身的 ok 缺省为成功；execute_command 额外以
    // exit_code 判定命令成败，保住「命令非零退出仍标红」的现有体验。
    let parsed = serde_json::from_str::<serde_json::Value>(result).ok();
    let exit_code = parsed
        .as_ref()
        .and_then(|v| v.get("exit_code"))
        .and_then(|v| v.as_i64());
    let ok = parsed
        .as_ref()
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
        && exit_code.map(|code| code == 0).unwrap_or(true);
    let error = if ok {
        None
    } else {
        parsed
            .as_ref()
            .and_then(|v| v.pointer("/error/message"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| exit_code.map(|code| format!("命令退出码为 {code}")))
    };
    // 参数摘要：优先取 query 字段，否则截断整个参数串。
    let summary = serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .and_then(|v| {
            v.get("query")
                .and_then(|q| q.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| arguments.chars().take(100).collect());
    // 参数与结果详情（截断）供前端展开查看完整输出。
    let arguments_detail: String = arguments.chars().take(1000).collect();
    let result_detail: String = result.chars().take(1000).collect();
    let payload = serde_json::json!({
        "call_id": call_id,
        "tool": tool,
        "ok": ok,
        "summary": summary,
        "error": error,
        "arguments": arguments_detail,
        "result": result_detail,
    });
    if let Err(e) = app.emit(event_names::AI_TOOL_CALL, &payload) {
        tracing::warn!("emit ai:tool_call 失败: {e}");
    }
    ok
}

/// 把 provider chunk 流包装成呈现层流（工具闭环外的路径不插入栅栏）。
fn presentation_stream(stream: ChunkStream) -> PresentationStream {
    Box::pin(stream.filter_map(|chunk| async move {
        match chunk {
            Ok(LlmChunk::ToolCalls(calls)) => {
                tracing::warn!(count = calls.len(), "非工具调用回复流包含工具调用，已丢弃");
                None
            },
            Ok(chunk) => Some(Ok(PresentationChunk::Chunk(chunk))),
            Err(e) => Some(Err(e)),
        }
    }))
}
