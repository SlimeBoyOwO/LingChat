//! 工具系统的运行部件：注册表、执行器、权限、设置、检索与工具闭环。
//!
//! 具体的工具定义在 [`builtin`] 子模块里；插件工具的接入见 `ling_chat_plugins`。

pub mod agent;
pub mod background_command;
pub mod builtin;
pub mod executor;
pub mod permissions;
pub mod registry;
pub mod settings;
pub mod tool_loop;
pub mod tool_rag;

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;

/// 从 AppHandle 获取共享的 `GameStatus` 句柄。
///
/// 锁顺序统一为 `ai_service.lock()` → `game_status.lock()`（与 api/scene.rs 等命令一致），
/// 避免嵌套死锁。clone 出句柄后即释放 ai_service 锁，工具再独立锁 game_status。
pub async fn game_status_handle(app: &AppHandle) -> Arc<Mutex<GameStatus>> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    service.game_status.clone()
}

/// 校验工具参数必须是空 object（无参工具共用）。
pub(crate) fn ensure_no_args(arguments: &Value, tool: &str) -> Result<(), String> {
    let Some(obj) = arguments.as_object() else {
        return Err(format!("{tool} 参数必须是 JSON object"));
    };
    if !obj.is_empty() {
        return Err(format!("{tool} 不接受参数"));
    }
    Ok(())
}

/// 将内容写到同目录临时文件，再原子替换目标文件。
/// `tempfile::persist` 在 Windows 上也支持覆盖已有目标，避免普通 `fs::rename`
/// 导致工具第一次写入成功、后续更新全部失败。
pub(crate) fn atomic_replace(path: &Path, content: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("目标路径没有父目录: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("创建目录 {} 失败: {e}", parent.display()))?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|e| format!("创建临时文件失败: {e}"))?;
    temporary
        .write_all(content)
        .map_err(|e| format!("写入临时文件失败: {e}"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| format!("同步临时文件失败: {e}"))?;
    temporary
        .persist(path)
        .map_err(|e| format!("替换 {} 失败: {}", path.display(), e.error))?;
    Ok(())
}
