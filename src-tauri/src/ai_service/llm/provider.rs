use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;

use crate::ai_service::llm::ChunkStream;
use crate::ai_service::types::LlmMessage;

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
}
