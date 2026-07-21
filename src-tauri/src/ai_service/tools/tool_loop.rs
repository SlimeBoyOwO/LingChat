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

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::ai_service::tools::clock::CurrentTimeTool;
    use crate::ai_service::types::{FunctionCall, ToolCall};

    struct MockStreamingProvider {
        supports_tools: bool,
        responses: Mutex<VecDeque<Vec<LlmChunk>>>,
        histories: Mutex<Vec<Vec<LlmMessage>>>,
    }

    impl MockStreamingProvider {
        fn new(supports_tools: bool, responses: Vec<Vec<LlmChunk>>) -> Self {
            Self {
                supports_tools,
                responses: Mutex::new(responses.into()),
                histories: Mutex::new(Vec::new()),
            }
        }

        fn next_stream(&self, messages: &[LlmMessage]) -> Result<ChunkStream> {
            self.histories.lock().unwrap().push(messages.to_vec());
            let chunks = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| anyhow!("mock 响应不足"))?;
            Ok(Box::pin(stream::iter(chunks.into_iter().map(Ok))))
        }
    }

    #[async_trait]
    impl StreamingToolProvider for MockStreamingProvider {
        fn supports_streaming_tools(&self) -> bool {
            self.supports_tools
        }

        async fn stream_with_tools(
            &self,
            messages: &[LlmMessage],
            _: &[ToolDefinition],
        ) -> Result<ChunkStream> {
            self.next_stream(messages)
        }

        async fn stream(&self, messages: &[LlmMessage]) -> Result<ChunkStream> {
            self.next_stream(messages)
        }
    }

    fn registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();
        registry.register(Arc::new(CurrentTimeTool)).unwrap();
        registry
    }

    fn call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            type_: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: "{}".to_string(),
            },
        }
    }

    async fn collect(stream: ChunkStream) -> Vec<LlmChunk> {
        stream.try_collect::<Vec<_>>().await.unwrap()
    }

    #[tokio::test]
    async fn text_response_uses_one_stream_request() {
        let provider =
            MockStreamingProvider::new(true, vec![vec![LlmChunk::Content("你好".to_string())]]);
        let result = collect(
            stream_with_tool_loop_with_provider(
                &provider,
                &registry(),
                vec![LlmMessage::user("你好")],
            )
            .await
            .unwrap(),
        )
        .await;

        assert!(matches!(&result[..], [LlmChunk::Content(text)] if text == "你好"));
        assert_eq!(provider.histories.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn tool_round_text_is_not_forwarded_and_next_round_is_returned() {
        let provider = MockStreamingProvider::new(
            true,
            vec![
                vec![
                    LlmChunk::Content("不应发布".to_string()),
                    LlmChunk::ToolCalls(vec![call("call-1", "get_current_time")]),
                ],
                vec![LlmChunk::Content("现在是下午".to_string())],
            ],
        );
        let result = collect(
            stream_with_tool_loop_with_provider(
                &provider,
                &registry(),
                vec![LlmMessage::user("几点")],
            )
            .await
            .unwrap(),
        )
        .await;

        assert!(matches!(&result[..], [LlmChunk::Content(text)] if text == "现在是下午"));
        let histories = provider.histories.lock().unwrap();
        assert_eq!(histories.len(), 2);
        assert_eq!(histories[1][1].role, "assistant");
        assert_eq!(histories[1][2].tool_call_id.as_deref(), Some("call-1"));
    }

    #[tokio::test]
    async fn preserves_multiple_call_order() {
        let provider = MockStreamingProvider::new(
            true,
            vec![
                vec![LlmChunk::ToolCalls(vec![
                    call("first", "get_current_time"),
                    call("second", "get_current_time"),
                ])],
                vec![LlmChunk::Content("完成".to_string())],
            ],
        );
        let _ = collect(
            stream_with_tool_loop_with_provider(
                &provider,
                &registry(),
                vec![LlmMessage::user("两次时间")],
            )
            .await
            .unwrap(),
        )
        .await;

        let histories = provider.histories.lock().unwrap();
        assert_eq!(histories[1][2].tool_call_id.as_deref(), Some("first"));
        assert_eq!(histories[1][3].tool_call_id.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn rejects_empty_and_duplicate_call_ids() {
        for calls in [
            vec![call("", "get_current_time")],
            vec![
                call("same", "get_current_time"),
                call("same", "get_current_time"),
            ],
        ] {
            let provider = MockStreamingProvider::new(true, vec![vec![LlmChunk::ToolCalls(calls)]]);
            let error = match stream_with_tool_loop_with_provider(
                &provider,
                &registry(),
                vec![LlmMessage::user("时间")],
            )
            .await
            {
                Ok(_) => panic!("应拒绝无效工具调用 ID"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("调用 ID"));
        }
    }

    #[tokio::test]
    async fn rejects_calls_beyond_maximum_rounds() {
        let responses = (0..=MAX_TOOL_ROUNDS)
            .map(|index| {
                vec![LlmChunk::ToolCalls(vec![call(
                    &format!("call-{index}"),
                    "get_current_time",
                )])]
            })
            .collect();
        let provider = MockStreamingProvider::new(true, responses);
        let error = match stream_with_tool_loop_with_provider(
            &provider,
            &registry(),
            vec![LlmMessage::user("持续调用")],
        )
        .await
        {
            Ok(_) => panic!("应拒绝超过最大轮次的工具调用"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("超过最大轮次 3"));
    }

    #[tokio::test]
    async fn unsupported_provider_uses_one_plain_stream() {
        let provider =
            MockStreamingProvider::new(false, vec![vec![LlmChunk::Content("普通流".to_string())]]);
        let result = collect(
            stream_with_tool_loop_with_provider(
                &provider,
                &registry(),
                vec![LlmMessage::user("你好")],
            )
            .await
            .unwrap(),
        )
        .await;
        assert!(matches!(&result[..], [LlmChunk::Content(text)] if text == "普通流"));
        assert_eq!(provider.histories.lock().unwrap().len(), 1);
    }
}
