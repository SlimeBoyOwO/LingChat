//! 按协议组织图片供应商实现，供工厂创建对应适配器。

mod codex;
pub(super) mod novelai;
mod openai;

pub(super) use codex::CodexImageProvider;
pub(super) use novelai::NovelAiProvider;
pub use novelai::NovelAiSettings;
pub(super) use openai::OpenAiImageProvider;
