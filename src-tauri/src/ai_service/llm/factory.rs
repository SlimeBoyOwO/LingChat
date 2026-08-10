use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;

use crate::utils::tls::build_tls_config;
use super::provider::LlmProvider;
use super::providers::{GenaiProvider, KimiCodeProvider};
use super::{LlmClient, LlmConfig};

/// 构建共享 reqwest Client。
///
/// reqwest 0.13 的 `rustls` feature 默认使用 `rustls-platform-verifier`
/// （验证操作系统证书）。这在 Android 上要求启动时显式初始化（JVM + Context +
/// ClassLoader），未初始化时 TLS 握手会 panic，导致 LLM 请求崩溃。
///
/// 这里改用 webpki-roots（内置 Mozilla CA 根证书）构造 rustls ClientConfig
/// 注入 reqwest（见 `utils::tls::build_tls_config`），绕开 platform-verifier
/// —— 全平台一致，无需任何初始化。
pub(crate) fn build_http_client(timeout_secs: u64) -> Result<Client> {
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
        }
        "kimicode" => Box::new(KimiCodeProvider::from_config(&cfg)?),
        // "webllm" 已废弃，原为 OpenAiProvider 别名，现统一用 "openai"
        other => return Err(anyhow!("不支持的 LLM 提供商: {other}")),
    };
    Ok(LlmClient::new(cfg, http, provider))
}
