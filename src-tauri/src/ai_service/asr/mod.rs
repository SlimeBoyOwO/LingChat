//! ASR (Automatic Speech Recognition) 服务。
//!
//! 端点检测由 [`vad::AsrVad`] 负责（本地 Silero ONNX）；
//! 识别交由 [`provider`] 的云 ASR provider 实现；
//! 会话编排由 [`session::AsrSession`] 统一管理互斥和取消；
//! 配置由 [`settings`] 通过 tauri_plugin_store 持久化。

pub mod error;
pub mod provider;
pub mod provider_stream;
pub mod session;
pub mod settings;
pub mod vad;
pub mod vad_segmenter;

use std::sync::Arc;
use tokio::sync::Mutex;

/// 全局 ASR 状态，由 `InnerAppState` 持有。
///
/// `session` 字段在 `init::initialize` 之前为 `None`；命令侧需自行处理"未初始化"。
pub struct AsrState {
    /// 当前活跃的 ASR 会话。`None` 表示未启动或 init 失败。
    /// 互斥：同一时刻最多一个 `AsrSource`（Button / Auto）。
    pub session: Arc<Mutex<Option<crate::ai_service::asr::session::AsrSession>>>,
}
