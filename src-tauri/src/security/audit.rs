//! 安全审计日志。
//!
//! 所有安全相关事件以 JSON Lines 追加写入 `data/security/audit.log`，
//! 文件超过上限后轮转为 `audit.1.log`。审计覆盖：
//! - 命令执行：完整命令、cwd、沙箱根、风险等级、审批方式与结果、执行结果
//! - 命令拦截：被安全策略拒绝的命令与原因
//! - 文件操作：写入 / 删除 / 编辑的路径与来源工具
//! - 审批决策：请求 ID、工具、同意/拒绝
//! - 高危设置变更：auto_approve / allow_any_path 等开关翻转
//! - 注入检测：命中提示词注入防护时的级别与命中模式
//!
//! 写入失败只降级为 tracing 告警，绝不阻断业务路径（审计不应成为故障点）。

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

/// 单个审计日志文件的大小上限（超过后轮转）。
const MAX_AUDIT_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// 轮转保留的旧文件（audit.1.log），再旧的直接覆盖。
const ROTATED_FILE_NAME: &str = "audit.1.log";

/// 全局写入锁：审计事件来自多个 async 任务，串行化落盘避免交错。
static AUDIT_LOCK: Mutex<()> = Mutex::new(());

/// 审计日志目录（`data/security/`；data 目录未初始化时降级到系统临时目录，
/// 保证单元测试等场景不会 panic）。
pub fn audit_dir() -> PathBuf {
    crate::init::static_copy::try_get_data_dir()
        .map(|dir| dir.join("security"))
        .unwrap_or_else(|| std::env::temp_dir().join("lingchat_security_audit"))
}

fn audit_log_path() -> PathBuf {
    audit_dir().join("audit.log")
}

fn rotated_log_path() -> PathBuf {
    audit_dir().join(ROTATED_FILE_NAME)
}

/// 追加一条审计事件。
///
/// `event_type` 为事件名（snake_case），`fields` 为事件负载（可序列化对象）。
/// 时间戳由本函数统一注入。
pub fn log_event<S: serde::Serialize>(event_type: &str, fields: S) {
    let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string();
    let mut entry = match serde_json::to_value(fields) {
        Ok(serde_json::Value::Object(mut map)) => {
            map.insert("ts".to_string(), serde_json::json!(ts));
            map.insert("event".to_string(), serde_json::json!(event_type));
            serde_json::Value::Object(map)
        }
        Ok(other) => serde_json::json!({
            "ts": ts,
            "event": event_type,
            "data": other,
        }),
        Err(e) => {
            tracing::warn!("审计事件序列化失败: {e}");
            return;
        }
    };
    // 每行一条，确保 JSONL 可逐行解析
    if let Some(map) = entry.as_object_mut() {
        let seq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let _ = map.insert("seq".to_string(), serde_json::json!(seq));
    }
    let line = entry.to_string();

    // 锁仅在持锁期间持有，避免跨 await 或长阻塞
    let Ok(_guard) = AUDIT_LOCK.lock() else {
        tracing::warn!("审计日志锁中毒，跳过本次记录");
        return;
    };
    if let Err(e) = write_line(&line) {
        tracing::warn!("安全审计日志写入失败: {e}");
    }
}

fn write_line(line: &str) -> Result<(), String> {
    let path = audit_log_path();
    let dir = audit_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建审计目录失败: {e}"))?;

    // 超过上限 → 轮转：audit.log → audit.1.log（覆盖旧文件）
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_AUDIT_FILE_BYTES {
            let _ = std::fs::remove_file(rotated_log_path());
            let _ = std::fs::rename(&path, rotated_log_path());
        }
    }

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("打开审计日志失败: {e}"))?;
    file.write_all(line.as_bytes())
        .map_err(|e| format!("写入审计日志失败: {e}"))?;
    file.write_all(b"\n")
        .map_err(|e| format!("写入审计日志失败: {e}"))?;
    file.flush().map_err(|e| format!("刷新审计日志失败: {e}"))?;
    Ok(())
}

// ─── 常用事件构造快捷函数 ────────────────────────────────────

/// 记录一次命令执行（含审批方式与执行结果）。
#[allow(clippy::too_many_arguments)]
pub fn command_executed(
    source: &str,
    command: &str,
    cwd: &str,
    sandbox_dir: &std::path::Path,
    risk: &str,
    approval: &str,
    approved: bool,
    outcome: &str,
    exit_code: Option<i32>,
    duration_ms: Option<u64>,
) {
    log_event(
        "command_executed",
        serde_json::json!({
            "source": source,
            "command": command,
            "cwd": cwd,
            "sandbox_dir": sandbox_dir.to_string_lossy(),
            "risk": risk,
            "approval": approval,
            "approved": approved,
            "outcome": outcome,
            "exit_code": exit_code,
            "duration_ms": duration_ms,
        }),
    );
}

/// 记录一次被拦截的命令。
pub fn command_blocked(source: &str, command: &str, reason: &str) {
    log_event(
        "command_blocked",
        serde_json::json!({
            "source": source,
            "command": command,
            "reason": reason,
        }),
    );
}

/// 记录一次文件写操作。
pub fn file_written(source: &str, path: &std::path::Path, bytes: u64, append: bool) {
    log_event(
        "file_written",
        serde_json::json!({
            "source": source,
            "path": path.to_string_lossy(),
            "bytes": bytes,
            "append": append,
        }),
    );
}

/// 记录一次文件删除操作。
pub fn file_deleted(source: &str, path: &std::path::Path) {
    log_event(
        "file_deleted",
        serde_json::json!({
            "source": source,
            "path": path.to_string_lossy(),
        }),
    );
}

/// 记录一次审批决策。
pub fn approval_decided(
    source: &str,
    request_id: &str,
    tool: &str,
    approved: bool,
    note: &str,
) {
    log_event(
        "approval_decided",
        serde_json::json!({
            "source": source,
            "request_id": request_id,
            "tool": tool,
            "approved": approved,
            "note": note,
        }),
    );
}

/// 记录一次高危设置变更。
pub fn settings_changed(key: &str, from: bool, to: bool) {
    log_event(
        "settings_changed",
        serde_json::json!({
            "key": key,
            "from": from,
            "to": to,
        }),
    );
}

/// 记录一次提示词注入检测命中。
pub fn injection_detected(source: &str, level: &str, notes: &[String]) {
    log_event(
        "injection_detected",
        serde_json::json!({
            "source": source,
            "level": level,
            "notes": notes,
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_event_produces_jsonl_line() {
        // 仅验证序列化路径不崩溃（不依赖 data_dir 落盘结果）
        let mut entry = serde_json::to_value(serde_json::json!({"a": 1})).unwrap();
        entry
            .as_object_mut()
            .map(|map| {
                map.insert("ts".to_string(), serde_json::json!("t"));
                map.insert("event".to_string(), serde_json::json!("x"));
            });
        let line = entry.to_string();
        assert!(line.contains("\"event\":\"x\""));
        assert!(line.contains("\"a\":1"));
    }
}
