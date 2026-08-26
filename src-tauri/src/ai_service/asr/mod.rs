//! ASR (Automatic Speech Recognition) 服务。
//!
//! 端点检测由 [`vad::AsrVad`] 负责（本地 Silero ONNX）；
//! 识别交由 [`provider`] 的云 ASR provider 实现；
//! 会话编排由 [`session::AsrSession`] 统一管理互斥和取消；
//! 配置由 [`settings`] 通过 tauri_plugin_store 持久化。

pub mod error;
pub mod provider;
pub mod provider_stream;
pub mod provider_stream_llama;
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
    /// 存 `Arc<AsrSession>` 而非本体：命令侧锁内 clone 引用（微秒级）后
    /// 锁外调用长耗时方法（30s 网络等待不阻塞其它 ASR 命令）。
    pub session: Arc<Mutex<Option<Arc<crate::ai_service::asr::session::AsrSession>>>>,
}
