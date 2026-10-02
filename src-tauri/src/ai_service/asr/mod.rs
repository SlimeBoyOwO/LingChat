//! ASR (Automatic Speech Recognition) 服务。
//!
//! 端点检测由 [`vad::AsrVad`] 负责（本地 Silero ONNX）；
//! 识别由 [`provider`] 的 [`provider::AsrProvider`] trait 抽象，具体实现见
//! [`providers`]（qwen 云 / llama 本地），注册与模型清单见 [`registry`]；
//! 会话编排由 [`session::AsrSession`] 统一管理互斥和取消；
//! 配置由 [`settings`] 通过 tauri_plugin_store 持久化。
//!
//! [`provider`] 只保留 trait 与对外重导出（历史路径 `asr::provider::X` 全部有效）：
//!
//! | 模块 | 职责 |
//! |---|---|
//! | [`provider_types`] | 对外类型：识别结果 / 调用参数 / 凭证 / 模型元数据 |
//! | [`provider_meta`] | provider 的 UI 配置元数据 |
//! | [`providers`] | 各 provider 实现，一实现一文件 |
//! | [`qwen_models`] | qwen 模型目录与协议能力表 |
//! | [`registry`] | 注册表 / 模型清单分发 / 流式参数 |
//! | [`config_fields`] | 设置页动态表单字段 |

pub mod config_fields;
pub mod debug_log;
pub mod error;
#[cfg(desktop)]
pub mod global_hotkey;
pub mod provider;
pub mod provider_meta;
pub mod provider_stream;
pub mod provider_stream_llama;
pub mod provider_types;
pub mod providers;
pub mod qwen_models;
pub mod region;
pub mod registry;
pub mod session;
pub mod settings;
pub mod vad;
pub mod vad_segmenter;

use std::sync::Arc;
use tokio::sync::Mutex;

/// 全局 ASR 状态，由 `InnerAppState` 持有。
///
/// `session` 字段在 `app::setup::asr::init_asr` 执行之前为 `None`；
/// 命令侧需自行处理"未初始化"。
pub struct AsrState {
    /// 当前活跃的 ASR 会话。`None` 表示未启动或 init 失败。
    /// 互斥：同一时刻最多一个 `AsrSource`（Button / Auto）。
    /// 存 `Arc<AsrSession>` 而非本体：命令侧锁内 clone 引用（微秒级）后
    /// 锁外调用长耗时方法（30s 网络等待不阻塞其它 ASR 命令）。
    pub session: Arc<Mutex<Option<Arc<crate::ai_service::asr::session::AsrSession>>>>,
}
