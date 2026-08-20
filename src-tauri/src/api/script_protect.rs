//! 内置剧本保护守卫。
//!
//! 剧本编辑器（`script_editor` 模块）本身不感知"受保护剧本"的概念——保护逻辑
//! 集中在该模块，以**同名命令**的形式接管 `editor_list_scripts` /
//! `editor_read_script` 的注册（见 lib.rs 的 invoke_handler）：
//! 列表过滤掉受保护剧本，读取直接拒绝。编辑器自身代码零改动。

use tauri::AppHandle;

use crate::api::script_editor::commands::{self, ScriptDetail, ScriptPackage};

/// 硬编码的受保护剧本（按叶子目录名匹配）。
/// 这些剧本随应用发布、含有编辑器不支持的特殊事件（jumpscare / force_choice 等），
/// 不对剧本编辑器开放：不出现在列表，也无法被编辑器读取打开。
const PROTECTED_SCRIPT_FOLDERS: [&str; 1] = ["第七个测试剧本"];

fn is_protected_script(folder_name: &str) -> bool {
    PROTECTED_SCRIPT_FOLDERS.contains(&folder_name)
}

/// 接管 `editor_list_scripts`：结果里过滤掉受保护剧本。
#[tauri::command]
pub async fn editor_list_scripts(app: AppHandle) -> Result<Vec<ScriptPackage>, String> {
    let mut scripts = commands::editor_list_scripts(app).await?;
    scripts.retain(|p| !is_protected_script(&p.folder_name));
    Ok(scripts)
}

/// 接管 `editor_read_script`：受保护剧本拒绝被编辑器打开。
#[tauri::command]
pub async fn editor_read_script(app: AppHandle, key: String) -> Result<ScriptDetail, String> {
    let detail = commands::editor_read_script(app, key).await?;
    if is_protected_script(&detail.package.folder_name) {
        return Err("该剧本为内置剧本，无法在剧本编辑器中打开".to_string());
    }
    Ok(detail)
}
