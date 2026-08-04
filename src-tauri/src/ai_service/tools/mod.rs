pub mod character;
pub mod clock;
pub mod executor;
pub mod memory;
pub mod permissions;
pub mod registry;
pub mod scene;
pub mod schedule;
pub mod settings;
pub mod status;
pub mod tool_loop;
pub mod web_search;

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::AppState;

use character::{CharacterList, CharacterSwitch};
use clock::CurrentTimeTool;
use memory::{AddNote, DeleteNote, GetCurrentMemory, GetNotes, UpdateNote};
use permissions::ToolPermissionConfig;
use permissions::CONFIG_FILE_NAME;
use registry::ToolRegistry;
use scene::{SceneList, SceneSwitch};
use schedule::{AddTodo, DeleteTodo, GetAllSchedule, UpdateTodo};
use settings::SharedToolSettings;
use status::{CurrentStatus, SceneStatus};
use web_search::WebSearchTool;

/// 从 AppHandle 获取共享的 `GameStatus` 句柄。
///
/// 锁顺序统一为 `ai_service.lock()` → `game_status.lock()`（与 api/scene.rs 等命令一致），
/// 避免嵌套死锁。clone 出句柄后即释放 ai_service 锁，工具再独立锁 game_status。
pub(crate) async fn game_status_handle(app: &AppHandle) -> Arc<Mutex<GameStatus>> {
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

/// 创建并注册所有内置聊天工具。
pub fn built_in_registry(
    role_names: impl IntoIterator<Item = (String, String)>,
    tool_settings: SharedToolSettings,
    app: tauri::AppHandle,
) -> Result<ToolRegistry> {
    let mut registry = ToolRegistry::new();
    registry.register(Arc::new(CurrentTimeTool))?;
    registry.register(Arc::new(WebSearchTool::new(tool_settings.clone(), app)))?;
    registry.register(Arc::new(GetAllSchedule))?;
    registry.register(Arc::new(AddTodo))?;
    registry.register(Arc::new(UpdateTodo))?;
    registry.register(Arc::new(DeleteTodo))?;
    registry.register(Arc::new(GetCurrentMemory))?;
    registry.register(Arc::new(GetNotes))?;
    registry.register(Arc::new(AddNote))?;
    registry.register(Arc::new(UpdateNote))?;
    registry.register(Arc::new(DeleteNote))?;
    registry.register(Arc::new(CurrentStatus))?;
    registry.register(Arc::new(SceneStatus))?;
    registry.register(Arc::new(SceneList))?;
    registry.register(Arc::new(SceneSwitch))?;
    registry.register(Arc::new(CharacterList))?;
    registry.register(Arc::new(CharacterSwitch))?;
    let data_dir = crate::api::data_dir();
    let mut permissions = ToolPermissionConfig::load_or_create(
        &data_dir,
        registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name),
    )?;
    permissions.set_available_tools(
        registry
            .definitions()
            .into_iter()
            .map(|definition| definition.function.name)
            .collect(),
    );
    permissions.initialize_characters(&data_dir, role_names.into_iter().map(|(_, name)| name))?;
    // 启动时按用户配置同步各工具权限：已启用的工具组/web_search 放开给
    // default 角色组（新建配置的 default 组默认关闭）。
    tool_settings.get().sync_to_permissions(&mut permissions);
    // 覆盖写 available_tools 展示列表（仅展示，运行时不被读取）
    permissions.save(&data_dir.join(CONFIG_FILE_NAME))?;
    registry.set_permissions(permissions);
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::executor::Tool;
    use super::*;

    #[test]
    fn atomic_replace_overwrites_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tool-data.json");
        atomic_replace(&path, b"first").unwrap();
        atomic_replace(&path, b"second").unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "second");
    }

    #[test]
    fn all_non_network_tool_schemas_are_strict_objects_and_uniquely_named() {
        let definitions = vec![
            CurrentTimeTool.definition(),
            GetAllSchedule.definition(),
            AddTodo.definition(),
            UpdateTodo.definition(),
            DeleteTodo.definition(),
            GetCurrentMemory.definition(),
            GetNotes.definition(),
            AddNote.definition(),
            UpdateNote.definition(),
            DeleteNote.definition(),
            CurrentStatus.definition(),
            SceneStatus.definition(),
            SceneList.definition(),
            SceneSwitch.definition(),
            CharacterList.definition(),
            CharacterSwitch.definition(),
        ];
        let mut names: HashSet<String> = HashSet::new();
        for definition in &definitions {
            assert!(names.insert(definition.function.name.clone()));
            assert_eq!(definition.function.parameters["type"], "object");
            assert_eq!(
                definition.function.parameters["additionalProperties"], false,
                "{} must reject unknown arguments",
                definition.function.name
            );
            assert!(definition.function.parameters["properties"].is_object());
            assert!(definition.function.parameters["required"].is_array());
        }
        assert_eq!(definitions.len(), 16);

        let grouped: HashSet<&str> = settings::TOOL_GROUPS
            .iter()
            .flat_map(|(_, tools)| tools.iter().copied())
            .collect();
        assert_eq!(grouped.len(), 16);
        assert!(names.iter().all(|name| grouped.contains(name.as_str())));
    }
}
