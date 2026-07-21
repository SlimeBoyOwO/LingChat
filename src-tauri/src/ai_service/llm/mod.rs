//! LLM client with provider abstraction.
//!
//! 对标 Python 版 `ling_chat/core/llm_providers/` 的工厂+ABC 模式。
//! `LlmClient` 是薄包装，具体协议由 `LlmProvider` trait 实现处理。

mod factory;
mod provider;
pub mod provider_config;
mod providers;

pub use factory::create_llm_client;
pub use provider::{LlmModelInfo, LlmProvider, LlmResponseWithTools};

use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_util::{Stream, StreamExt};
use reqwest::Client;
use tokio::sync::RwLock;

use crate::ai_service::types::{LlmMessage, ToolDefinition};

// ============================================================
// SharedLlmClient —— 支持运行时热替换的 LLM 客户端槽位
// ============================================================

/// 可热替换的 LLM 客户端槽位。
///
/// 外层 `Arc` 允许多处共享同一个槽位；内层 `RwLock<Option<Arc<LlmClient>>>`
/// 允许在运行时原子地替换内部客户端，而不需要重启应用。
///
/// - **读取**：调用 `snapshot()` 获取当前 `Arc<LlmClient>` 的快照，
///   之后所有操作都基于该快照，不受后续热切换影响。
/// - **替换**：调用 `swap()` 写入新的客户端，旧客户端的 `Arc` 引用
///   在所有持有者释放后自然回收。
pub type LlmSlot = Arc<RwLock<Option<Arc<LlmClient>>>>;

/// 从 `LlmSlot` 异步读取当前客户端快照。
///
/// 返回 `Option<Arc<LlmClient>>`，`None` 表示尚未配置可用模型。
pub async fn slot_snapshot(slot: &LlmSlot) -> Option<Arc<LlmClient>> {
    slot.read().await.clone()
}

/// 运行时 LLM 配置。
#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
    pub timeout_secs: u64,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub enable_thinking: bool,
    /// 推理深度（如 "low" / "high" / "max"），由支持 reasoning 的模型使用（如 Kimi Code K3 系列）。
    pub reasoning_effort: Option<String>,
}

impl LlmConfig {
    pub fn is_usable(&self) -> bool {
        !self.api_key.is_empty() && !self.model.is_empty()
    }
}

/// LLM 流式返回的一个片段：可能是正式回复内容，也可能是思考链内容。
#[derive(Debug, Clone)]
pub enum LlmChunk {
    /// 正式回复内容（会被前端显示并加入记忆）。
    Content(String),
    /// 思考链内容（仅用于实时统计，不加入正式回复）。
    Reasoning(String),
    /// 一轮流式请求结束后得到的完整工具调用，仅供工具闭环内部消费。
    ToolCalls(Vec<crate::ai_service::types::ToolCall>),
}

pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<LlmChunk>> + Send>>;

/// LLM 客户端：薄包装，把协议细节委托给内部的 `LlmProvider`。
pub struct LlmClient {
    cfg: LlmConfig,
    http: Client,
    provider: Box<dyn LlmProvider>,
}

impl LlmClient {
    pub fn new(cfg: LlmConfig, http: Client, provider: Box<dyn LlmProvider>) -> Self {
        Self {
            cfg,
            http,
            provider,
        }
    }

    pub fn config(&self) -> &LlmConfig {
        &self.cfg
    }

    pub async fn list_models(&self) -> Result<Vec<LlmModelInfo>> {
        self.provider.list_models(&self.http).await
    }

    /// 非流式：一次性取完整回复。
    pub async fn complete(&self, messages: &[LlmMessage]) -> Result<String> {
        if !self.cfg.is_usable() {
            return Err(anyhow!("LLM 未配置 API key 或 model"));
        }
        self.provider.complete(&self.http, messages).await
    }

    /// 流式：返回 `AsyncStream<Result<LlmChunk>>`。每个元素是一段内容或思考链片段。
    pub async fn complete_stream(&self, messages: &[LlmMessage]) -> Result<ChunkStream> {
        self.complete_stream_inner(messages, None).await
    }

    /// 是否支持原生流式 function calling。
    pub fn supports_streaming_tools(&self) -> bool {
        self.provider.supports_streaming_tools()
    }

    /// 流式 + function calling。
    pub async fn complete_stream_with_tools(
        &self,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<ChunkStream> {
        self.complete_stream_inner(messages, Some((tools, tool_choice)))
            .await
    }

    async fn complete_stream_inner(
        &self,
        messages: &[LlmMessage],
        tools: Option<(&[ToolDefinition], Option<&str>)>,
    ) -> Result<ChunkStream> {
        if !self.cfg.is_usable() {
            return Err(anyhow!("LLM 未配置 API key 或 model"));
        }
        let mut inner = match tools {
            Some((definitions, tool_choice)) => {
                self.provider
                    .complete_stream_with_tools(&self.http, messages, definitions, tool_choice)
                    .await?
            }
            None => self.provider.complete_stream(&self.http, messages).await?,
        };
        let timeout_secs = self.cfg.timeout_secs;
        let idle_timeout = Duration::from_secs(timeout_secs);
        let stream = async_stream::try_stream! {
            loop {
                let item = tokio::time::timeout(idle_timeout, inner.next())
                    .await
                    .map_err(|_| anyhow!("LLM 流式响应空闲超时（{timeout_secs} 秒）"))?;
                match item {
                    Some(chunk) => yield chunk?,
                    None => break,
                }
            }
        };
        Ok(Box::pin(stream))
    }

    /// 非流式 + function calling。
    pub async fn complete_with_tools(
        &self,
        messages: &[LlmMessage],
        tools: &[ToolDefinition],
        tool_choice: Option<&str>,
    ) -> Result<LlmResponseWithTools> {
        if !self.cfg.is_usable() {
            return Err(anyhow!("LLM 未配置 API key 或 model"));
        }
        self.provider
            .complete_with_tools(&self.http, messages, tools, tool_choice)
            .await
    }
}
