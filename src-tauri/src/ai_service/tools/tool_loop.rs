use std::collections::HashSet;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::{stream, StreamExt};
use tokio::sync::mpsc;

use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmClient};
use crate::ai_service::message_system::generator::GeneratorSource;
use crate::ai_service::types::{LlmMessage, ToolDefinition};

use super::executor::{ToolContext, ToolExecutor};
use super::registry::ToolRegistry;

const MAX_TOOL_ROUNDS: usize = 3;

#[async_trait]
trait StreamingToolProvider: Send + Sync {
    fn supports_streaming_tools(&self) -> bool;

    async fn stream_with_tools(
        &self,
        messages: &[LlmMessage],
        definitions: &[ToolDefinition],
    ) -> Result<ChunkStream>;

    async fn stream(&self, messages: &[LlmMessage]) -> Result<ChunkStream>;
}

#[async_trait]
impl StreamingToolProvider for LlmClient {
    fn supports_streaming_tools(&self) -> bool {
        self.supports_streaming_tools()
    }

    async fn stream_with_tools(
        &self,
        messages: &[LlmMessage],
        definitions: &[ToolDefinition],
    ) -> Result<ChunkStream> {
        self.complete_stream_with_tools(messages, definitions, Some("auto"))
            .await
    }

    async fn stream(&self, messages: &[LlmMessage]) -> Result<ChunkStream> {
        self.complete_stream(messages).await
    }
}

pub struct ToolLoopResult {
    pub stream: ChunkStream,
    pub tool_messages: Vec<LlmMessage>,
}

/// 以流式请求执行普通聊天的工具闭环。
///
/// 仅支持原生流式 tools 的 provider 会携带工具定义请求。工具调用必须等到本轮
/// 流结束后才会执行，以确保参数已经由 provider 合并完整。其他 provider 保持
/// 单次普通流式请求，避免退回非流式预检。
pub async fn stream_with_tool_loop(
    llm: &LlmClient,
    registry: &ToolRegistry,
    messages: Vec<LlmMessage>,
    source: GeneratorSource,
    role_name: Option<String>,
) -> Result<ToolLoopResult> {
    stream_with_tool_loop_with_provider(llm, registry, messages, source, role_name).await
}

async fn stream_with_tool_loop_with_provider(
    provider: &dyn StreamingToolProvider,
    registry: &ToolRegistry,
    mut messages: Vec<LlmMessage>,
    source: GeneratorSource,
    role_name: Option<String>,
) -> Result<ToolLoopResult> {
    let allowed = registry.allowed_tools(source, role_name.as_deref());
    let definitions = registry.definitions_for_allowed(&allowed);
    if definitions.is_empty() || !provider.supports_streaming_tools() {
        if !definitions.is_empty() {
            tracing::info!("当前 LLM Provider 不支持原生流式工具调用，跳过普通聊天工具闭环");
        }
        return Ok(ToolLoopResult {
            stream: presentation_stream(provider.stream(&messages).await?),
            tool_messages: Vec::new(),
        });
    }

    let executor = ToolExecutor::new(registry);
    let context = ToolContext::new(allowed);
    let mut tool_messages = Vec::new();

    // 用 channel 把每轮的 Content chunk 实时透传出去，最终 stream 包含所有轮次的内容
    let (content_tx, content_rx) = mpsc::unbounded_channel::<LlmChunk>();

    for round in 0..=MAX_TOOL_ROUNDS {
        tracing::info!(round = round + 1, "开始流式聊天工具决策");
        let mut response_stream = provider.stream_with_tools(&messages, &definitions).await?;
        let mut tool_calls = Vec::new();
        let mut round_text = String::new();

        while let Some(chunk) = response_stream.next().await {
            match chunk? {
                LlmChunk::ToolCalls(calls) => tool_calls.extend(calls),
                LlmChunk::Content(text) => {
                    round_text.push_str(&text);
                    let _ = content_tx.send(LlmChunk::Content(text));
                }
                // Thinking 等其它 chunk 也一起透传
                other => {
                    let _ = content_tx.send(other);
                }
            }
        }

        if tool_calls.is_empty() {
            // 本轮没有工具调用，工具闭环结束，关闭 channel 并返回合并 stream
            drop(content_tx);
            return Ok(ToolLoopResult {
                stream: Box::pin(content_rx_stream(content_rx)),
                tool_messages,
            });
        }
        if round == MAX_TOOL_ROUNDS {
            drop(content_tx);
            return Err(anyhow!("工具调用超过最大轮次 {MAX_TOOL_ROUNDS}"));
        }

        let calls = tool_calls;

        let mut ids = HashSet::new();
        for call in &calls {
            if call.id.trim().is_empty() {
                return Err(anyhow!("Provider 返回了空工具调用 ID"));
            }
            if !ids.insert(call.id.clone()) {
                return Err(anyhow!("Provider 返回了重复工具调用 ID: {}", call.id));
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
        tool_messages.push(assistant_message);
        for call in calls {
            tracing::info!(tool = call.function.name, call_id = call.id, "执行聊天工具");
            let result = executor
                .execute(&call.function.name, &call.function.arguments, &context)
                .await;
            let tool_result = LlmMessage::tool_result(call.id, result);
            messages.push(tool_result.clone());
            tool_messages.push(tool_result);
        }
    }

    unreachable!("工具循环必须在限定轮次内返回")
}

/// 将 mpsc receiver 转为 ChunkStream。
fn content_rx_stream(rx: mpsc::UnboundedReceiver<LlmChunk>) -> ChunkStream {
    Box::pin(stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Some(chunk) => Some((Ok(chunk), rx)),
            None => None,
        }
    }))
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

