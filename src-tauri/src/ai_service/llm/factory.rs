use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;

use super::provider::LlmProvider;
use super::providers::{GenaiProvider, KimiCodeProvider};
use super::{LlmClient, LlmConfig};

/// 根据 `cfg.provider` 创建对应的 LLM 客户端。
pub fn create_llm_client(cfg: LlmConfig) -> Result<LlmClient> {
    let http = Client::builder()
        .read_timeout(Duration::from_secs(cfg.timeout_secs))
        .build()
        .context("创建 LLM HTTP 客户端失败")?;
    let provider: Box<dyn LlmProvider> = match cfg.provider.to_lowercase().as_str() {
        "deepseek" | "openai" | "lmstudio" | "gemini" => {
            Box::new(GenaiProvider::new(&cfg, http.clone())?)
        }
        "kimicode" => Box::new(KimiCodeProvider::from_config(&cfg)?),
        // "webllm" 已废弃，原为 OpenAiProvider 别名，现统一用 "openai"
        other => return Err(anyhow!("不支持的 LLM 提供商: {other}")),
    };
    Ok(LlmClient::new(cfg, http, provider))
}
