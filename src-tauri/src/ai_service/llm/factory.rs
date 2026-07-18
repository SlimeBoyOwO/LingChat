use anyhow::{anyhow, Result};

use super::provider::LlmProvider;
use super::providers::{GenaiProvider, KimiCodeProvider};
use super::{LlmClient, LlmConfig};

/// 根据 `cfg.provider` 创建对应的 LLM 客户端。
pub fn create_llm_client(cfg: LlmConfig) -> Result<LlmClient> {
    let provider: Box<dyn LlmProvider> = match cfg.provider.to_lowercase().as_str() {
        "deepseek" | "openai" | "lmstudio" | "gemini" => {
            Box::new(GenaiProvider::new(&cfg)?)
        }
        "kimicode" => Box::new(KimiCodeProvider::from_config(&cfg)?),
        // "webllm" 已废弃，原为 OpenAiProvider 别名，现统一用 "openai"
        other => return Err(anyhow!("不支持的 LLM 提供商: {other}")),
    };
    LlmClient::new(cfg, provider)
}
