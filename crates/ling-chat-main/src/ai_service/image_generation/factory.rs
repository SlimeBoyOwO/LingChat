//! 图片供应商工厂与界面目录，集中创建对应协议的适配器。
//! 认证、模型发现和生成请求由各供应商实现，公共服务只依赖统一接口。

use anyhow::{Result, bail};

use super::ImageGenerationSettings;
use super::provider::{ImageProvider, ImageProviderInfo};
use super::providers::{CodexImageProvider, NovelAiProvider, OpenAiImageProvider};

pub(super) const DEFAULT_PROVIDER: &str = "codex";

pub(super) fn create_image_provider(
    settings: &ImageGenerationSettings,
) -> Result<Box<dyn ImageProvider>> {
    let provider: Box<dyn ImageProvider> = match settings.provider.as_str() {
        "codex" => Box::new(CodexImageProvider::new(settings)),
        "openai-compatible" => Box::new(OpenAiImageProvider::new(settings)),
        "novelai" => Box::new(NovelAiProvider::new(settings)),
        other => bail!("不支持的图片生成供应商：{other}，请重新选择"),
    };
    Ok(provider)
}

pub fn list_providers() -> Vec<ImageProviderInfo> {
    vec![
        ImageProviderInfo {
            id: DEFAULT_PROVIDER,
            label: "Codex 订阅",
            custom: false,
            protocol: "codex",
        },
        ImageProviderInfo {
            id: "openai-compatible",
            label: "OpenAI compatible",
            custom: true,
            protocol: "openai",
        },
        ImageProviderInfo {
            id: "novelai",
            label: "NovelAI",
            custom: false,
            protocol: "novelai",
        },
    ]
}
