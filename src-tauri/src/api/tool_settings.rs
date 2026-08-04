//! 聊天工具的用户配置命令（网页搜索等）。

use tauri::Manager;

use crate::ai_service::tools::executor::{Tool, ToolContext};
use crate::ai_service::tools::permissions::CONFIG_FILE_NAME;
use crate::ai_service::tools::settings::ToolSettings;
use crate::ai_service::tools::web_search::WebSearchTool;
use crate::AppState;

/// 读取当前工具配置。
#[tauri::command]
pub async fn get_tool_settings(app: tauri::AppHandle) -> Result<ToolSettings, String> {
    let state = app.state::<AppState>();
    Ok(state.tool_settings.get())
}

/// 保存工具配置：写盘 + 热更新 + 同步权限矩阵。
///
/// 网页搜索「启用且配好 API Key」时，自动放开 default 角色组的
/// `web_search` 权限（新建权限配置中 default 组默认全关）；关闭时收回。
#[tauri::command]
pub async fn save_tool_settings(
    app: tauri::AppHandle,
    settings: ToolSettings,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let data_dir = super::data_dir();
    settings.save(&data_dir).map_err(|e| e.to_string())?;
    state.tool_settings.update(settings.clone());

    let web_search_ready = settings.web_search.is_ready();
    state.tool_registry.update_permissions(|permissions| {
        permissions.set_tool_allowed_for_default_group("web_search", web_search_ready);
        if let Err(e) = permissions.save(&data_dir.join(CONFIG_FILE_NAME)) {
            tracing::warn!("保存工具权限配置失败: {e}");
        }
    });
    Ok(())
}

/// 直接执行一次网页搜索（供设置页「测试搜索」按钮使用）。
#[tauri::command]
pub async fn test_web_search(app: tauri::AppHandle, query: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let tool = WebSearchTool::new(state.tool_settings.clone(), app.clone());
    let context = ToolContext::new(["web_search".to_string()].into_iter().collect());
    let result = tool
        .execute(&context, serde_json::json!({ "query": query }))
        .await
        .map_err(|e| e.to_string())?;
    Ok(result.to_string())
}
