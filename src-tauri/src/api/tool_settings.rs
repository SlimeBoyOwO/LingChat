//! 聊天工具的用户配置命令（网页搜索等）。

use tauri::Manager;

use crate::ai_service::skill_agent::command_executor;
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
    mut settings: ToolSettings,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let data_dir = super::data_dir();
    settings.normalize();
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

/// 返回当前 LingChat 进程是否已通过 Windows UAC 获得管理员令牌。
#[tauri::command]
pub fn get_tool_elevation_status() -> bool {
    command_executor::is_current_process_elevated()
}

/// 使用 Windows 正常 RunAs 流程启动管理员实例；成功启动后退出当前标准权限实例。
#[tauri::command]
pub async fn restart_tool_process_as_admin(app: tauri::AppHandle) -> Result<(), String> {
    if command_executor::is_current_process_elevated() {
        return Ok(());
    }
    tokio::task::spawn_blocking(command_executor::launch_current_process_as_admin)
        .await
        .map_err(|error| format!("管理员重启任务异常: {error}"))?
        .map_err(|error| error.to_string())?;
    // 使用独立系统线程而不是 Tauri async runtime：`app.exit(0)` 会关闭窗口并触发
    // 退出存档，但某些仍在运行的后台任务可能让进程继续残留。重启场景下必须保证
    // 旧 PID 最终释放，否则已提权的辅助进程会一直等不到启动新实例的时机。
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.exit(0);
        std::thread::sleep(std::time::Duration::from_secs(5));
        tracing::warn!("管理员重启时旧进程未在宽限期内退出，正在结束残留进程");
        std::process::exit(0);
    });
    Ok(())
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

/// 主聊天文件写入/编辑审批回调。
#[tauri::command]
pub async fn resolve_file_change_approval(
    app: tauri::AppHandle,
    request_id: String,
    approved: bool,
) -> Result<(), String> {
    tracing::info!("[approval] resolve_file_change_approval 收到回传: request_id={request_id} approved={approved}");
    let state = app.state::<AppState>();
    let request = state
        .chat_file_change_approvals
        .lock()
        .await
        .remove(&request_id);
    match request {
        Some(request) => {
            let _ = request.tx.send(approved);
            Ok(())
        }
        None => {
            tracing::warn!(
                "[approval] resolve_file_change_approval 未找到请求: request_id={request_id}"
            );
            Err("文件修改审批请求不存在或已过期".into())
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
