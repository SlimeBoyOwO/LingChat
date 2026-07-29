//! 角色压缩包导入/导出的全局并发状态与 RAII 守卫。
//!
//! `RoleArchiveState` 由 Tauri 管理；导入命令通过 `ImportingGuard` 与
//! `TaskRemoveGuard` 自动释放并发锁、清理缓存副本。

use std::path::PathBuf;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// 单个导入任务的运行时状态。
/// `saf_cache_path` 用于在取消任务时立即清理 SAF 缓存副本。
pub struct ImportTaskEntry {
    pub cancel_token: Arc<CancellationToken>,
    pub saf_cache_path: std::sync::Mutex<Option<PathBuf>>,
}

/// 角色压缩包导入/导出的全局状态。
/// - `tasks`：当前正在运行的导入任务，键为任务 ID。
/// - `importing`：全局导入并发锁，为 `true` 时拒绝新任务。
pub struct RoleArchiveState {
    pub tasks: std::sync::Mutex<std::collections::HashMap<String, ImportTaskEntry>>,
    pub importing: std::sync::atomic::AtomicBool,
}

impl Default for RoleArchiveState {
    fn default() -> Self {
        Self {
            tasks: std::sync::Mutex::new(std::collections::HashMap::new()),
            importing: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

/// 基于 RAII 的守卫，函数返回时自动释放 `importing` 标志。
pub(crate) struct ImportingGuard<'a> {
    pub(crate) flag: &'a std::sync::atomic::AtomicBool,
}

/// 基于 RAII 的守卫，函数返回时自动移除任务并清理 SAF 缓存副本。
pub(crate) struct TaskRemoveGuard<'a> {
    pub(crate) state: &'a RoleArchiveState,
    pub(crate) task_id: &'a str,
}

impl Drop for ImportingGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for TaskRemoveGuard<'_> {
    fn drop(&mut self) {
        if let Ok(mut tasks) = self.state.tasks.lock() {
            if let Some(entry) = tasks.remove(self.task_id) {
                if let Ok(mut guard) = entry.saf_cache_path.lock() {
                    if let Some(path) = guard.take() {
                        let _ = std::fs::remove_file(&path);
                    }
                }
            }
        }
    }
}
