//! LLM 请求体日志模块
//!
//! 记录每次对 LLM 发送的完整请求体到 `data/log/llm/` 目录下。
//! 文件名格式：`{YYYYMMDD_HHMMSS}_{provider}_{序列号:05}.json`
//!
//! 由 `log.llm_request_body` 设置控制开关，默认关闭。

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::OnceLock;

use chrono::Local;

/// 全局开关（由设置控制，默认关闭）
static ENABLED: AtomicBool = AtomicBool::new(false);

/// LLM 请求日志输出目录
static LOG_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

/// 请求计数器，用于文件名中的序列号
static REQUEST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// 初始化 LLM 请求日志模块
///
/// 在 `data_dir/log/llm/` 下创建日志目录。
/// `enable` 为 `false` 时仍会创建目录结构，但不写入日志。
pub fn init(data_dir: &Path, enable: bool) {
    let log_dir = data_dir.join("log").join("llm");
    let _ = fs::create_dir_all(&log_dir);
    LOG_DIR.set(log_dir).ok();
    ENABLED.store(enable, Ordering::Release);
    if enable {
        tracing::info!("LLM 请求体日志已启用，输出目录: data/log/llm/");
    }
}

/// 记录 LLM 请求体到文件
///
/// 如果开关未开启或目录未初始化，直接返回。
pub fn log_request_body(provider_name: &str, body: &serde_json::Value) {
    if !ENABLED.load(Ordering::Acquire) {
        return;
    }
    let Some(log_dir) = LOG_DIR.get() else { return };

    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = log_dir.join(format!("{}_{}_{:05}.json", timestamp, provider_name, counter));

    let formatted = serde_json::to_string_pretty(body).unwrap_or_default();
    if let Err(e) = fs::write(&filename, &formatted) {
        tracing::warn!("写入 LLM 请求体日志失败: {e}");
    }
}
