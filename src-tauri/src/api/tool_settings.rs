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

    // 同步权限矩阵：启用的工具组/web_search 放开给 default 角色组，关闭的收回
    state.tool_registry.update_permissions(|permissions| {
        settings.sync_to_permissions(permissions);
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

/// 主聊天 `execute_command` 的审批回调：前端弹窗后把用户决定送回等待中的工具。
#[tauri::command]
pub async fn resolve_command_approval(
    app: tauri::AppHandle,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    tracing::info!("[approval] resolve_command_approval 收到回传: request_id={request_id} approved={approved}");
    let state = app.state::<AppState>();
    let request = state
        .chat_command_approvals
        .lock()
        .await
        .remove(&request_id);
    match request {
        Some(request) => {
            let _ = request.tx.send(approved);
            Ok(())
        }
        None => {
            tracing::warn!("[approval] resolve_command_approval 未找到请求: request_id={request_id}");
            Err("审批请求不存在或已过期".into())
        }
    }
}

/// 主聊天 `delete_file` 的审批回调：前端确认后把决定送回等待中的删除工具。
#[tauri::command]
pub async fn resolve_file_delete_approval(
    app: tauri::AppHandle,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    tracing::info!("[approval] resolve_file_delete_approval 收到回传: request_id={request_id} approved={approved}");
    let state = app.state::<AppState>();
    let request = state
        .chat_file_delete_approvals
        .lock()
        .await
        .remove(&request_id);
    match request {
        Some(request) => {
            let _ = request.tx.send(approved);
            Ok(())
        }
        None => {
            tracing::warn!("[approval] resolve_file_delete_approval 未找到请求: request_id={request_id}");
            Err("删除审批请求不存在或已过期".into())
        }
    }
}
