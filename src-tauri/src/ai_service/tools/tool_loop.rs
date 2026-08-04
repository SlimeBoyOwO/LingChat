use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmClient};
use crate::ai_service::message_system::generator::GeneratorSource;
use crate::ai_service::message_system::responses::event_names;
use crate::ai_service::types::LlmMessage;

use super::executor::{ToolContext, ToolExecutor};
use super::registry::ToolRegistry;

const MAX_TOOL_ROUNDS: usize = 3;

/// 工具消息收集槽：流消费过程中由闭环填充，消费完毕后调用方取走。
pub type ToolMessageSink = Arc<Mutex<Vec<LlmMessage>>>;

pub struct ToolLoopResult {
    pub stream: ChunkStream,
    pub tool_messages: ToolMessageSink,
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
pub async fn stream_with_tool_loop(
    llm: &Arc<LlmClient>,
    registry: &Arc<ToolRegistry>,
    messages: Vec<LlmMessage>,
    source: GeneratorSource,
    role_name: Option<String>,
    app: &AppHandle,
) -> Result<ToolLoopResult> {
    let allowed = registry.allowed_tools(source, role_name.as_deref());
    let definitions = registry.definitions_for_allowed(&allowed);
    if definitions.is_empty() || !llm.supports_streaming_tools() {
        if !definitions.is_empty() {
            tracing::info!("当前 LLM Provider 不支持原生流式工具调用，跳过普通聊天工具闭环");
        }
        return Ok(ToolLoopResult {
            stream: presentation_stream(llm.complete_stream(&messages).await?),
            tool_messages: Arc::new(Mutex::new(Vec::new())),
        });
    }

    let tool_messages: ToolMessageSink = Arc::new(Mutex::new(Vec::new()));
    let sink = tool_messages.clone();

    // 流需要 'static：闭环状态全部改为持有所有权（Arc/克隆）
    let llm = llm.clone();
    let registry = registry.clone();
    let app = app.clone();

    let stream = async_stream::try_stream! {
        let executor = ToolExecutor::new(&registry);
        let context = ToolContext::new(allowed).with_app(app.clone());
        let mut messages = messages;

        for round in 0..=MAX_TOOL_ROUNDS {
            tracing::info!(round = round + 1, "开始流式聊天工具决策");
            let mut response_stream = llm
                .complete_stream_with_tools(&messages, &definitions, Some("auto"))
                .await?;
            let mut tool_calls = Vec::new();
            let mut round_text = String::new();

            while let Some(chunk) = response_stream.next().await {
                match chunk? {
                    LlmChunk::ToolCalls(calls) => tool_calls.extend(calls),
                    LlmChunk::Content(text) => {
                        round_text.push_str(&text);
                        // 实时透传，保持下游流式体验
                        yield LlmChunk::Content(text);
                    }
                    // Thinking 等其它 chunk 也一起实时透传
                    other => {
                        yield other;
                    }
                }
            }

            if tool_calls.is_empty() {
                // 本轮没有工具调用，工具闭环结束
                return;
            }
            if round == MAX_TOOL_ROUNDS {
                Err(anyhow!("工具调用超过最大轮次 {MAX_TOOL_ROUNDS}"))?;
            }

            let calls = tool_calls;

            let mut ids = HashSet::new();
            for call in &calls {
                if call.id.trim().is_empty() {
                    Err(anyhow!("Provider 返回了空工具调用 ID"))?;
                }
                if !ids.insert(call.id.clone()) {
                    Err(anyhow!("Provider 返回了重复工具调用 ID: {}", call.id))?;
                }
            }

            // 本轮的内容文本 + tool_calls 一起存入助理消息
            let assistant_message = LlmMessage {
                role: "assistant".to_string(),
                content: round_text,
                tool_calls: Some(calls.clone()),
                tool_call_id: None,
            };
            messages.push(assistant_message.clone());
            sink.lock().await.push(assistant_message);
            for call in calls {
                tracing::info!(tool = call.function.name, call_id = call.id, "执行聊天工具");
                let result = executor
                    .execute(&call.function.name, &call.function.arguments, &context)
                    .await;
                emit_tool_call_event(&app, &call.function.name, &call.function.arguments, &result);
                let tool_result = LlmMessage::tool_result(call.id, result);
                messages.push(tool_result.clone());
                sink.lock().await.push(tool_result);
            }
        }
    };

    Ok(ToolLoopResult {
        stream: Box::pin(stream),
        tool_messages,
    })
}

/// 工具执行后向前端广播 `ai:tool_call` 事件（用于调用提示/通知）。
fn emit_tool_call_event(
    app: &AppHandle,
    tool: &str,
    arguments: &str,
    result: &str,
) {
    // executor 的可恢复错误统一编码为 {"ok": false, "error": {...}}；
    // 成功结果没有 "ok" 字段（或显式 "ok": true）。
    let parsed = serde_json::from_str::<serde_json::Value>(result).ok();
    let ok = parsed
        .as_ref()
        .and_then(|v| v.get("ok"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let error = if ok {
        None
    } else {
        parsed
            .as_ref()
            .and_then(|v| v.pointer("/error/message"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
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
    let payload = serde_json::json!({
        "tool": tool,
        "ok": ok,
        "summary": summary,
        "error": error,
    });
    if let Err(e) = app.emit(event_names::AI_TOOL_CALL, &payload) {
        tracing::warn!("emit ai:tool_call 失败: {e}");
    }
}

fn presentation_stream(stream: ChunkStream) -> ChunkStream {
    Box::pin(stream.filter_map(|chunk| async move {
        match chunk {
            Ok(LlmChunk::ToolCalls(calls)) => {
                tracing::warn!(count = calls.len(), "非工具调用回复流包含工具调用，已丢弃");
                None
            }
            chunk => Some(chunk),
        }
    }))
}
