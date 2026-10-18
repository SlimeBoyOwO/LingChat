//! LLM 请求体日志模块
//!
//! 两种输出：
//! - 文件日志：每次请求的完整请求体写入 `data/log/llm/` 目录下，
//!   文件名格式 `{YYYYMMDD_HHMMSS}_{provider}_{序列号:05}.json`，
//!   由 `log.llm_request_body` 设置控制开关，默认关闭；
//! - 内存快照：保留最近若干条请求的可读摘要，恒开、不落盘，
//!   供调试指令 `/查看原始记忆` 查看「拼装完成后实际下发的上下文」。
//!   一轮回复之后还会跟着好感度评估、发言人判定、记忆压缩等后台请求，
//!   只留最后一条必然被它们挤掉，所以按环形缓冲留多条。

use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

use chrono::Local;

/// 全局开关（由设置控制，默认关闭）
static ENABLED: AtomicBool = AtomicBool::new(false);

/// LLM 请求日志输出目录
static LOG_DIR: OnceLock<std::path::PathBuf> = OnceLock::new();

/// 请求计数器，用于文件名中的序列号
static REQUEST_COUNTER: AtomicU32 = AtomicU32::new(0);

/// 最近若干条下发请求的摘要（可读文本，旧 → 新）。
static RECENT_REQUESTS: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// 环形缓冲容量。一轮回复会连着发出主对话、发言人判定、好感度评估、记忆压缩等多条请求，
/// 留够一轮有余的深度，才回溯得到想看的那条主对话。
const MAX_RECENT_REQUESTS: usize = 12;

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

/// 将模型名等片段清洗成合法的文件名片段
///
/// 模型名可能包含路径分隔符（`/`、`\`）或其他 Windows 文件名非法字符，
/// 直接拼进文件名会导致写入失败或落到不存在的子目录。
fn sanitize_for_filename(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_control() || r#"\/:*?"<>|"#.contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    if cleaned.is_empty() {
        "-".to_string()
    } else {
        cleaned
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
    let filename = log_dir.join(format!(
        "{}_{}_{:05}.json",
        timestamp,
        sanitize_for_filename(provider_name),
        counter
    ));

    let formatted = serde_json::to_string_pretty(body).unwrap_or_default();
    if let Err(e) = fs::write(&filename, &formatted) {
        tracing::warn!("写入 LLM 请求体日志失败: {e}");
    }
}

/// 文件日志开关是否开启。调用方据此决定要不要做请求体序列化
/// （请求可能带 base64 图片，无谓的序列化代价不低）。
pub fn file_log_enabled() -> bool {
    ENABLED.load(Ordering::Acquire)
}

/// 压入一条请求摘要。摘要由 provider 侧渲染（只有那边拿得到结构化类型）。
pub fn push_request_summary(rendered: String) {
    let mut queue = RECENT_REQUESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if queue.len() >= MAX_RECENT_REQUESTS {
        queue.pop_front();
    }
    queue.push_back(rendered);
}

/// 取最近若干条请求摘要，**最新的在前**（看日志时最关心的那条离顶部最近）。
pub fn recent_request_summaries() -> Vec<String> {
    RECENT_REQUESTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .rev()
        .cloned()
        .collect()
}
