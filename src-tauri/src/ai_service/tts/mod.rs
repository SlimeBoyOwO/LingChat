//! TTS 子系统：对应 Python `ling_chat.core.TTS` + `voice_maker.py`。
//!
//! - `TtsAdapter`：适配器 trait（对一次合成请求抽象），每个后端一个实现。
//! - `TtsProvider`：持有所有已初始化的适配器，根据 `tts_type` 路由。
//! - `VoiceMaker`：角色级包装，持有 provider + 输出目录 + 语言，暴露
//!   `generate_voice_files(segments)`；与旧版接口一致。
//!
//! 所有 adapter 使用全局共享的 `reqwest::Client`（连接池复用）。
//! 写音频文件到磁盘的目录由 `set_temp_dir` 决定，默认 `<data>/voice/`。

mod adapters;
pub mod provider;
pub mod voice_maker;

pub use provider::{TtsAdapter, TtsProvider};
pub use voice_maker::VoiceMaker;
