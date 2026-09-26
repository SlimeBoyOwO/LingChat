//! ASR 调试日志开关。
//!
//! 与 [`crate::utils::llm_request_logger`] 同一个模式：模块内原子量 + setter，
//! 业务代码只问 [`enabled`]。开关值来自 `AsrSettings.vad_debug_log`，写入点两处：
//! 启动装配（`app/setup/asr.rs`）与设置保存（`api/asr.rs` 的 `asr_set_settings`）。
//!
//! 目前只有一项：逐帧 VAD 能量检测（frame/prob/len）。它录音期间每秒一条，是
//! 排查「语音识别为什么不触发」最直接的证据，正常使用纯属噪音，故默认关闭。

use std::sync::atomic::{AtomicBool, Ordering};

/// 是否输出逐帧 VAD 能量检测日志。
static VAD_FRAME_LOG: AtomicBool = AtomicBool::new(false);

/// 应用设置值。
pub fn set(enabled: bool) {
    VAD_FRAME_LOG.store(enabled, Ordering::Relaxed);
}

/// 逐帧 VAD 日志是否开启。调用点在每帧推理路径上（30ms 一次），`Relaxed` 足够
/// ——它只控制一条日志的取舍，不参与任何同步。
pub fn enabled() -> bool {
    VAD_FRAME_LOG.load(Ordering::Relaxed)
}
