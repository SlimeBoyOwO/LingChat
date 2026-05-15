use anyhow::{anyhow, Result};

use super::provider::LlmProvider;
use super::providers::{GeminiProvider, OpenAiProvider};
use super::{LlmClient, LlmConfig};

/// 根据 `cfg.provider` 创建对应的 LLM 客户端。
///
/// 对标 Python `LLMProviderFactory.create_provider()`。
pub fn create_llm_client(cfg: LlmConfig) -> Result<LlmClient> {
    let provider: Box<dyn LlmProvider> = match cfg.provider.to_lowercase().as_str() {
        "" | "openai" | "webllm" => Box::new(OpenAiProvider::from_config(&cfg)?),
        "gemini" => Box::new(GeminiProvider::from_config(&cfg)?),
        other => return Err(anyhow!("不支持的 LLM 提供商: {other}")),
    };
    LlmClient::new(cfg, provider)
}
