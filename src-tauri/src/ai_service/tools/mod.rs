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

use std::sync::Arc;

use anyhow::Result;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::AppState;

use character::{CharacterList, CharacterSwitch};
use clock::CurrentTimeTool;
use permissions::CONFIG_FILE_NAME;
use memory::{AddNote, DeleteNote, GetCurrentMemory, GetNotes, UpdateNote};
use permissions::ToolPermissionConfig;
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
    permissions.initialize_characters(
        &data_dir,
        role_names.into_iter().map(|(_, name)| name),
    )?;
    // 启动时按用户配置同步网页搜索权限：已启用且配置就绪时，
    // 确保 default 角色组放开 web_search（新建配置的 default 组默认关闭）。
    let web_search_ready = tool_settings.get().web_search.is_ready();
    permissions.set_tool_allowed_for_default_group("web_search", web_search_ready);
    // 覆盖写 available_tools 展示列表（仅展示，运行时不被读取）
    permissions.save(&data_dir.join(CONFIG_FILE_NAME))?;
    registry.set_permissions(permissions);
    Ok(registry)
}
