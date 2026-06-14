use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

use crate::ai_service::llm::ChunkStream;
use crate::ai_service::types::{LlmMessage, ToolCall, ToolDefinition};

/// `complete_with_tools` 的返回值。
#[derive(Debug, Clone)]
pub struct LlmResponseWithTools {
    /// 文本回复（可能为空，如果 LLM 只返回 tool call）。
    pub content: Option<String>,
    /// LLM 请求调用的工具列表。
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// LLM 供应商协议：不同供应商的唯一区别在于 HTTP 请求/响应的格式。
///
/// 对标 Python `BaseLLMProvider` ABC。
/// 参照 `TtsAdapter` trait 使用 `async_trait` + `Send + Sync` 的模式。
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 非流式：发送消息列表，返回完整回复文本。
    async fn complete(&self, http: &Client, messages: &[LlmMessage]) -> Result<String>;

    /// 流式：返回逐字符（或逐 token）的 chunk 流。
    async fn complete_stream(
        &self,
        http: &Client,
        messages: &[LlmMessage],
    ) -> Result<ChunkStream>;

    /// 非流式 + function calling。
    ///
    /// 默认实现 fallback 到 `complete()`（不支持 tools 的供应商）。
    async fn complete_with_tools(
        &self,
        http: &Client,
        messages: &[LlmMessage],
        _tools: &[ToolDefinition],
        _tool_choice: Option<&str>,
    ) -> Result<LlmResponseWithTools> {
        let text = self.complete(http, messages).await?;
        Ok(LlmResponseWithTools {
            content: Some(text),
            tool_calls: None,
        })
    }
}
