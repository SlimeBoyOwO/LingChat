//! 压缩包导入的全局并发状态与 RAII 守卫（角色 / 插件共用）。
//!
//! `ArchiveImportState` 由 Tauri 管理，导入命令通过 `ImportingGuard` 与
//! `TaskRemoveGuard` 自动释放并发锁、清理缓存副本。全局只允许一个导入任务：
//! 前端进度条同一时刻也只展示一个，跨领域并行反而会让取消按钮找不到目标。

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

/// 单个导入任务的运行时状态。
/// `saf_cache_path` 用于在取消任务时立即清理 SAF 缓存副本。
pub struct ImportTaskEntry {
    pub cancel_token: Arc<CancellationToken>,
    pub saf_cache_path: Mutex<Option<PathBuf>>,
}

/// 压缩包导入的全局状态。
/// - `tasks`：当前正在运行的导入任务，键为任务 ID。
/// - `importing`：全局导入并发锁，为 `true` 时拒绝新任务。
pub struct ArchiveImportState {
    pub tasks: Mutex<std::collections::HashMap<String, ImportTaskEntry>>,
    pub importing: AtomicBool,
}

impl Default for ArchiveImportState {
    fn default() -> Self {
        Self {
            tasks: Mutex::new(std::collections::HashMap::new()),
            importing: AtomicBool::new(false),
        }
    }
}

impl ArchiveImportState {
    /// 登记一个导入任务并返回其取消令牌。
    pub fn register_task(&self, task_id: &str) -> Arc<CancellationToken> {
        let cancel_token = Arc::new(CancellationToken::new());
        self.tasks.lock().unwrap().insert(
            task_id.to_string(),
            ImportTaskEntry {
                cancel_token: cancel_token.clone(),
                saf_cache_path: Mutex::new(None),
            },
        );
        cancel_token
    }

    /// 取出任务并立即清理其 SAF 缓存副本（取消路径与 RAII 守卫共用）。
    fn take_task(&self, task_id: &str) -> Option<ImportTaskEntry> {
        let entry = self.tasks.lock().unwrap().remove(task_id)?;
        if let Some(path) = entry.saf_cache_path.lock().unwrap().take() {
            let _ = std::fs::remove_file(&path);
        }
        Some(entry)
    }

    /// 记录任务在 Android SAF 下产生的缓存副本，供取消时立即回收。
    pub fn set_saf_cache(&self, task_id: &str, path: PathBuf) {
        if let Some(entry) = self.tasks.lock().unwrap().get_mut(task_id) {
            *entry.saf_cache_path.lock().unwrap() = Some(path);
        }
    }

    /// 请求取消指定任务；找不到任务时静默返回（守卫可能已先一步清理）。
    pub fn cancel_task(&self, task_id: &str) {
        if let Some(entry) = self.take_task(task_id) {
            entry.cancel_token.cancel();
        }
    }
}

/// 基于 RAII 的守卫，函数返回时自动释放 `importing` 标志。
pub(crate) struct ImportingGuard<'a> {
    pub(crate) flag: &'a AtomicBool,
}

impl Drop for ImportingGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// 基于 RAII 的守卫，函数返回时自动移除任务并清理 SAF 缓存副本。
pub(crate) struct TaskRemoveGuard<'a> {
    pub(crate) state: &'a ArchiveImportState,
    pub(crate) task_id: &'a str,
}

impl Drop for TaskRemoveGuard<'_> {
    fn drop(&mut self) {
        self.state.take_task(self.task_id);
    }
}
