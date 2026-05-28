//! LLM client with provider abstraction.
//!
//! 对标 Python 版 `ling_chat/core/llm_providers/` 的工厂+ABC 模式。
//! `LlmClient` 是薄包装，具体协议由 `LlmProvider` trait 实现处理。

mod factory;
mod provider;
mod providers;

pub use factory::create_llm_client;
pub use provider::LlmProvider;

use std::pin::Pin;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use futures_util::Stream;
use reqwest::Client;

use crate::ai_service::types::LlmMessage;

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
}

impl LlmConfig {
    pub fn is_usable(&self) -> bool {
        !self.api_key.is_empty() && !self.model.is_empty()
    }
}

pub type ChunkStream = Pin<Box<dyn Stream<Item = Result<String>> + Send>>;

/// LLM 客户端：薄包装，把协议细节委托给内部的 `LlmProvider`。
pub struct LlmClient {
    cfg: LlmConfig,
    http: Client,
    provider: Box<dyn LlmProvider>,
}

impl LlmClient {
    pub fn new(cfg: LlmConfig, provider: Box<dyn LlmProvider>) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(cfg.timeout_secs.max(10)))
            .build()
            .context("创建 LLM HTTP 客户端失败")?;
        Ok(Self { cfg, http, provider })
    }

    pub fn config(&self) -> &LlmConfig {
        &self.cfg
    }

    /// 非流式：一次性取完整回复。
    pub async fn complete(&self, messages: &[LlmMessage]) -> Result<String> {
        if !self.cfg.is_usable() {
            return Err(anyhow!("LLM 未配置 API key 或 model"));
        }
        self.provider.complete(&self.http, messages).await
    }

    /// 流式：返回 `AsyncStream<Result<String>>`。每个元素是一个字符 chunk。
    pub async fn complete_stream(&self, messages: &[LlmMessage]) -> Result<ChunkStream> {
        if !self.cfg.is_usable() {
            return Err(anyhow!("LLM 未配置 API key 或 model"));
        }
        self.provider.complete_stream(&self.http, messages).await
    }
}
