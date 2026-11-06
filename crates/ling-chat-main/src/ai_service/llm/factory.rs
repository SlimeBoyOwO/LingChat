use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use reqwest::Client;

use super::provider::LlmProvider;
use super::providers::{CodexProvider, GenaiProvider, KimiCodeProvider};
use super::{LlmClient, LlmConfig};
use crate::utils::tls::build_tls_config;

/// 构建共享 reqwest Client。
pub fn build_http_client(timeout_secs: u64) -> Result<Client> {
    let tls_config = build_tls_config().map_err(anyhow::Error::msg)?;
    Client::builder()
        .read_timeout(Duration::from_secs(timeout_secs))
        .tls_backend_preconfigured(tls_config)
        .build()
        .context("创建 LLM HTTP 客户端失败")
}

/// 根据 `cfg.provider` 创建对应的 LLM 客户端。
pub fn create_llm_client(cfg: LlmConfig) -> Result<LlmClient> {
    let http = build_http_client(cfg.timeout_secs)?;
    let provider: Box<dyn LlmProvider> = match cfg.provider.to_lowercase().as_str() {
        "deepseek" | "openai" | "lmstudio" | "gemini" => {
            Box::new(GenaiProvider::new(&cfg, http.clone())?)
        },
        "kimicode" => Box::new(KimiCodeProvider::from_config(&cfg)?),
        "codex" => Box::new(CodexProvider::from_config(&cfg)?),
        // "webllm" 已废弃，原为 OpenAiProvider 别名，现统一用 "openai"
        other => return Err(anyhow!("不支持的 LLM 提供商: {other}")),
    };
    Ok(LlmClient::new(cfg, http, provider))
}
