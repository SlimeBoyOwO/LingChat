//! ASR (Automatic Speech Recognition) 服务。
//!
//! 端点检测由 [`vad::AsrVad`] 负责（本地 Silero ONNX）；
//! 识别交由 [`provider`] 的云 ASR provider 实现；
//! 会话编排由 [`session::AsrSession`] 统一管理互斥和取消；
//! 配置由 [`settings`] 通过 tauri_plugin_store 持久化。

pub mod error;
pub mod provider;
pub mod settings;
pub mod vad;

pub use error::AsrError;