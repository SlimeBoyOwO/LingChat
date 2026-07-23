use std::collections::HashSet;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use futures_util::{stream, StreamExt};

use crate::ai_service::llm::{ChunkStream, LlmChunk, LlmClient};
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

/// 以流式请求执行普通聊天的工具闭环。
///
/// 仅支持原生流式 tools 的 provider 会携带工具定义请求。工具调用必须等到本轮
/// 流结束后才会执行，以确保参数已经由 provider 合并完整。其他 provider 保持
/// 单次普通流式请求，避免退回非流式预检。
pub async fn stream_with_tool_loop(
    llm: &LlmClient,
    registry: &ToolRegistry,
    messages: Vec<LlmMessage>,
) -> Result<ChunkStream> {
    stream_with_tool_loop_with_provider(llm, registry, messages).await
}

async fn stream_with_tool_loop_with_provider(
    provider: &dyn StreamingToolProvider,
    registry: &ToolRegistry,
    mut messages: Vec<LlmMessage>,
) -> Result<ChunkStream> {
    let definitions = registry.definitions();
    if definitions.is_empty() || !provider.supports_streaming_tools() {
        if !definitions.is_empty() {
            tracing::info!("当前 LLM Provider 不支持原生流式工具调用，跳过普通聊天工具闭环");
        }
        return provider.stream(&messages).await;
    }

    let executor = ToolExecutor::new(registry);
    let context = ToolContext;

    for round in 0..=MAX_TOOL_ROUNDS {
        tracing::info!(round = round + 1, "开始流式聊天工具决策");
        let mut response_stream = provider.stream_with_tools(&messages, &definitions).await?;
        let mut chunks = Vec::new();
        let mut tool_calls = None;

        while let Some(chunk) = response_stream.next().await {
            match chunk? {
                LlmChunk::ToolCalls(calls) => tool_calls = Some(calls),
                chunk => chunks.push(chunk),
            }
        }

        let calls = tool_calls.unwrap_or_default();
        if calls.is_empty() {
            return Ok(Box::pin(stream::iter(chunks.into_iter().map(Ok))));
        }
        if round == MAX_TOOL_ROUNDS {
            return Err(anyhow!("工具调用超过最大轮次 {MAX_TOOL_ROUNDS}"));
        }

        let mut ids = HashSet::new();
        for call in &calls {
            if call.id.trim().is_empty() {
                return Err(anyhow!("Provider 返回了空工具调用 ID"));
            }
            if !ids.insert(call.id.clone()) {
                return Err(anyhow!("Provider 返回了重复工具调用 ID: {}", call.id));
            }
        }

        messages.push(LlmMessage::tool(calls.clone()));
        for call in calls {
            tracing::info!(tool = call.function.name, call_id = call.id, "执行聊天工具");
            let result = executor
                .execute(&call.function.name, &call.function.arguments, &context)
                .await;
            messages.push(LlmMessage::tool_result(call.id, result));
        }
    }

    unreachable!("工具循环必须在限定轮次内返回")
}
