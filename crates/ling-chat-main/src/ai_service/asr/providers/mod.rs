//! ASR provider 的具体实现：一个 provider 一个文件。
//!
//! 新增 provider 的步骤见 `crate::ai_service::asr::provider` 的模块文档。

pub mod llama;
pub mod qwen;

pub use llama::LlamaAsrProvider;
pub use qwen::QwenAsrProvider;
