//! ASR (Automatic Speech Recognition) 服务。
//!
//! 本模块负责把语音（WAV 字节）转成文字。三阶段拆分：
//!
//! - [`error`] — 统一错误类型（含 i18n 码）。
//! - [`provider`] — 云 ASR provider 抽象（OpenAI Whisper / Qwen ASR / Gemini / LAN Whisper）。
//!
//! 端点检测 (vad) 与会话编排 (session) / 持久化设置 (settings) 在后续 Task 引入。

pub mod error;
pub mod provider;

pub use error::AsrError;