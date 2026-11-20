//! 图片供应商统一协议，以及模型目录、界面信息和提示词能力的共享类型。

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::Serialize;

use super::GenerateImageRequest;
use super::providers::novelai::NovelAiMetadata;

#[derive(Serialize)]
pub struct ImageProviderInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub custom: bool,
    pub protocol: &'static str,
}

#[derive(Clone, Serialize)]
pub struct ImageModelCatalog {
    pub models: Vec<String>,
    pub source: String,
    pub warning: String,
    pub allow_automatic: bool,
    pub novelai_metadata: Option<NovelAiMetadata>,
}

pub struct ImagePromptHints {
    pub description: &'static str,
    pub prompt: &'static str,
    pub negative_prompt: Option<&'static str>,
}

impl Default for ImagePromptHints {
    fn default() -> Self {
        Self {
            description: "按用户要求生成图片并加入背景/场景库。使用用户在生图设置中选择的供应商和模型，不能在参数中改写凭据、端点或模型。仅当用户希望切换到生成的场景时设置 apply_background。",
            prompt: "要生成的图片的详细描述",
            negative_prompt: None,
        }
    }
}

/// 供应商负责认证、模型发现和生成协议，公共服务负责图片校验与场景保存。
#[async_trait]
pub(super) trait ImageProvider: Send + Sync {
    fn validate(&self) -> Result<()> {
        Ok(())
    }

    fn prompt_hints(&self) -> ImagePromptHints {
        ImagePromptHints::default()
    }

    async fn list_models(&self, http: &Client, refresh: bool) -> Result<ImageModelCatalog>;

    async fn generate(
        &self,
        http: &Client,
        model: &str,
        request: &GenerateImageRequest,
        reference: Option<&[u8]>,
    ) -> Result<String>;
}
