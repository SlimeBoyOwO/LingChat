use crate::story::channels::NativeChannels;
use ling_chat_main::AppState;
use serde::Serialize;
use tauri::{AppHandle, Manager};
/// One preset-only persistent main-menu effect selected by the last script run.
/// The response contains no paths, CSS or HTML and is therefore safe to bind in
/// the shell menu.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ScriptMenuEffectResponse {
    pub theme: String,
    pub message: Option<String>,
    /// 特效归属剧本的 path_key（如 `standalone/第七个测试剧本`）；
    /// 前端据此在主题激活时把所有主菜单入口劫持到该剧本。
    pub owner: String,
}

/// 删角色文件彩蛋的"幽灵锁定"状态（DDLC ghost menu 的对应物）：
/// 玩家把剧本声明的角色标记 .chr 全部删掉后，进入该剧本不再走正常流程，
/// 而是锁成黑白幽灵立绘演出，只剩重置按钮可操作。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ScriptGhostLockResponse {
    pub locked: bool,
    /// Optional story-authored terminal error; checked before any entry effects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry_error: Option<String>,
    /// 锁定中时为该剧本 Assets 目录的绝对路径，前端用 convertFileSrc 加载
    /// 幽灵立绘/音效；未锁定为 None。不含文件名校验——素材缺失时前端静默降级。
    pub asset_dir: Option<String>,
}

#[tauri::command]
pub async fn get_script_menu_effect(app: AppHandle) -> Option<ScriptMenuEffectResponse> {
    let app_state = app.state::<AppState>();
    let service = app_state.ai_service.lock().await;
    let Some(state) = crate::story::events::menu_effect_event::read_menu_effect(&service.data_dir)
    else {
        return None;
    };

    // A DLC may also be removed manually while LingChat is closed. Never leave
    // an orphaned title skin behind when its owning script no longer exists.
    let owner_exists = service
        .script_manager
        .all_scripts
        .values()
        .any(|script| script.path_key() == state.owner);
    if !owner_exists {
        if let Err(error) =
            crate::story::events::menu_effect_event::clear_menu_effect(&service.data_dir)
        {
            tracing::warn!("[ScriptAPI] 清理孤立主菜单特效失败: {error:#}");
        }
        return None;
    }

    Some(ScriptMenuEffectResponse {
        theme: state.theme,
        message: state.message,
        owner: state.owner,
    })
}

/// 进入剧本前的幽灵锁定检查（DDLC ghost menu 的对应物）：玩家把该剧本声明的
/// 角色标记 .chr 全部删除后返回 locked=true 并带上 Assets 目录；玩家放回任一
/// .chr 或重置记忆（运行状态被清掉）后自动解锁。
#[tauri::command]
pub async fn check_script_ghost_lock(
    app: AppHandle,
    script_name: String,
) -> ScriptGhostLockResponse {
    let app_state = app.state::<AppState>();
    let service = app_state.ai_service.lock().await;
    let not_locked = || ScriptGhostLockResponse {
        locked: false,
        entry_error: None,
        asset_dir: None,
    };
    let Some(script) = service.script_manager.all_scripts.get(&script_name) else {
        return not_locked();
    };
    let entry_error = crate::story::persistent_state::entry_error(
        &service.data_dir,
        &script.path_key(),
        &script.settings,
    )
    .unwrap_or_else(|error| Some(format!("无法读取剧本运行状态: {error}")));
    if entry_error.is_some() {
        return ScriptGhostLockResponse {
            locked: false,
            entry_error,
            asset_dir: None,
        };
    }
    if !crate::story::events::menu_effect_event::script_markers_wiped(&service.data_dir, script) {
        return not_locked();
    }
    ScriptGhostLockResponse {
        locked: true,
        entry_error: None,
        asset_dir: Some(
            script
                .script_path
                .join("Assets")
                .to_string_lossy()
                .to_string(),
        ),
    }
}

/// Display one auxiliary horror window only after the frontend event queue has
/// reached its Rust-validated one-time ticket.
#[tauri::command]
pub async fn show_script_glitch_window(app: AppHandle, request_id: u64) -> Result<(), String> {
    crate::story::events::glitch_window_event::show_pending_glitch_window(&app, request_id)
        .await
        .map_err(|error| error.to_string())
}

/// Queue-ordered natural completion and immediate stop/error paths share this
/// idempotent cleanup command. It closes both Tauri glitch windows and native
/// TaskDialog/CMD/Notepad staging owned by the current script run.
#[tauri::command]
pub fn close_script_glitch_windows(app: AppHandle) {
    crate::story::events::glitch_window_event::close_all_glitch_windows(&app);
    super::script_popups::close_all();
}

/// Consume one Rust-validated, single-use, run-owned ticket at the exact
/// frontend queue beat, then open TaskDialog, Notepad, or real CMD without
/// launching PowerShell/pwsh. Replayed/stale/free-form invokes are rejected.
#[tauri::command]
pub async fn spawn_script_console_window(_app: AppHandle, request_id: u64) -> Result<(), String> {
    super::script_popups::show_pending(request_id)
}

/// Clear one script's persisted runtime state (playthrough memory), so the
/// next entry starts from the first-run route again. Refused while any script
/// is still running to avoid yanking state out from under a live run.
#[tauri::command]
pub async fn reset_script_state(app: AppHandle, script_name: String) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let service = state.ai_service.lock().await;
    if service
        .script_manager
        .is_running
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Err("剧本正在运行，请先退出再重置记忆".to_string());
    }
    let script = service
        .script_manager
        .all_scripts
        .get(&script_name)
        .ok_or_else(|| format!("剧本不存在: '{}'", script_name))?;
    let owner = script.path_key();

    // Snapshot every boundary artifact, then durably journal reset intent before
    // detaching progress. Ordinary I/O failures restore exact bytes/state; a
    // hard kill is completed generically at startup for any compatible script.
    let marker_snapshot =
        crate::story::events::character_file_event::snapshot_declared_character_files(
            script,
            &service.data_dir,
        )
        .map_err(|e| format!("准备角色文件重置快照失败: {:#}", e))?;
    let menu_snapshot =
        crate::story::events::menu_effect_event::snapshot_menu_effect_file(&service.data_dir)
            .map_err(|e| format!("准备菜单特效重置快照失败: {:#}", e))?;
    let reset_record = crate::story::reset_transaction::begin_reset(&service.data_dir, &owner)
        .map_err(|e| format!("创建持久重置事务失败: {e:#}"))?;
    let state_backup =
        match crate::story::persistent_state::take_playthrough(&service.data_dir, &owner) {
            Ok(backup) => backup,
            Err(error) => {
                // atomic_replace may have changed the state before a parent-dir
                // flush reported failure. Keep durable intent so startup can
                // finish the requested reset rather than deleting evidence.
                return Err(format!(
                    "准备重置剧本记忆失败，已保留恢复事务供下次启动重试: {error:#}"
                ));
            },
        };

    let rollback_boundaries = || {
        let mut failures = Vec::new();
        if let Err(error) =
            crate::story::events::character_file_event::restore_character_files_snapshot(
                script,
                &service.data_dir,
                &marker_snapshot,
            )
        {
            failures.push(format!("角色标记回滚失败: {error:#}"));
        }
        if let Err(error) = crate::story::events::menu_effect_event::restore_menu_effect_snapshot(
            &service.data_dir,
            &menu_snapshot,
        ) {
            failures.push(format!("菜单特效回滚失败: {error:#}"));
        }
        if let Some(backup) = state_backup.clone() {
            if let Err(error) = crate::story::persistent_state::restore_playthrough(
                &service.data_dir,
                &owner,
                backup,
            ) {
                failures.push(format!("周目状态回滚失败: {error:#}"));
            }
        }
        if failures.is_empty() {
            if let Err(error) =
                crate::story::reset_transaction::finish_reset(&service.data_dir, &reset_record)
            {
                failures.push(format!("重置事务回滚退休失败: {error:#}"));
            }
        } else {
            failures.push("已保留持久重置事务，启动恢复器会完成请求".to_string());
        }
        failures
    };
    let reset_error = |message: String| {
        let rollback_failures = rollback_boundaries();
        if rollback_failures.is_empty() {
            message
        } else {
            format!("{}；{}", message, rollback_failures.join("；"))
        }
    };

    let restored =
        match crate::story::events::character_file_event::restore_declared_character_files(
            script,
            &service.data_dir,
        ) {
            Ok(restored) => restored,
            Err(error) => return Err(reset_error(format!("恢复剧本角色文件失败: {error:#}"))),
        };
    let menu_cleared = match crate::story::events::menu_effect_event::clear_menu_effect_for_owner(
        &service.data_dir,
        &owner,
    ) {
        Ok(cleared) => cleared,
        Err(error) => return Err(reset_error(format!("清除剧本菜单特效失败: {error:#}"))),
    };
    crate::story::reset_transaction::finish_reset(&service.data_dir, &reset_record).map_err(
        |error| format!("重置已完成，但持久事务退休失败；下次启动会安全重放: {error:#}"),
    )?;
    Ok(state_backup.is_some() || restored > 0 || menu_cleared)
}

async fn validate_force_choice_warp_ticket(
    app: &AppHandle,
    request_id: &str,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    let guard = channels
        .extension_mut::<NativeChannels>()
        .force_choice_guard
        .as_mut()
        .ok_or("当前没有可牵引鼠标的 force_choice")?;
    if guard.request_id != request_id {
        return Err("force_choice 鼠标票据无效或已过期".to_string());
    }
    if !guard.warp_enabled {
        return Err("force_choice 鼠标牵引已取消".to_string());
    }
    if std::time::Instant::now() > guard.warp_expires_at {
        guard.warp_enabled = false;
        return Err("force_choice 鼠标牵引已超过 5 秒安全时限".to_string());
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct ScriptCursorPosition {
    pub x: f64,
    pub y: f64,
}

/// 读取系统指针在主窗口客户区内的逻辑坐标，作为 DDLC RigMouse 每次牵引的真实起点。
/// 多显示器可出现负的屏幕坐标，因此先减客户区原点再除 DPI，最后限制在视口边界。
#[tauri::command]
pub async fn get_script_cursor_position(
    window: tauri::WebviewWindow,
    request_id: String,
) -> Result<ScriptCursorPosition, String> {
    if window.label() != "main" {
        return Err("只有主窗口可以启动 force_choice 鼠标牵引".to_string());
    }
    validate_force_choice_warp_ticket(window.app_handle(), &request_id).await?;
    if !window.is_focused().map_err(|e| e.to_string())? {
        return Err("主窗口未聚焦，拒绝牵引系统鼠标".to_string());
    }
    let cursor = window.cursor_position().map_err(|e| e.to_string())?;
    let origin = window.inner_position().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let max_x = f64::from(size.width) / scale;
    let max_y = f64::from(size.height) / scale;
    Ok(ScriptCursorPosition {
        x: ((cursor.x - f64::from(origin.x)) / scale).clamp(0.0, max_x),
        y: ((cursor.y - f64::from(origin.y)) / scale).clamp(0.0, max_y),
    })
}

/// 把系统鼠标指针拖动到窗口内的指定 CSS 坐标。
///
/// 用于剧本的 `force_choice` 演出（DDLC 式强制拖动鼠标）。前端传视口 CSS 像素，
/// 这里只需换算成物理像素：**不能再叠加 `inner_position`**——tao 的
/// `set_cursor_position` 收的就是"窗口客户区相对坐标"，内部会自己做
/// ClientToScreen（Windows）/加窗口原点（macOS、Linux）。之前叠加了一次
/// inner_position，窗口非最大化时鼠标会被多拽出一段窗口偏移，方向看着就是歪的。
#[tauri::command]
pub async fn warp_cursor(
    window: tauri::WebviewWindow,
    request_id: String,
    x: f64,
    y: f64,
) -> Result<(), String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    // 诊断计数：前几次调用写 INFO 日志，便于排查"拖动没生效/方向不对"类反馈
    static LOG_COUNT: AtomicUsize = AtomicUsize::new(0);

    if window.label() != "main" {
        return Err("只有主窗口可以执行 force_choice 鼠标牵引".to_string());
    }
    validate_force_choice_warp_ticket(window.app_handle(), &request_id).await?;
    if !x.is_finite() || !y.is_finite() {
        return Err("鼠标目标坐标必须是有限数值".to_string());
    }
    if !window.is_focused().map_err(|e| e.to_string())? {
        return Err("主窗口未聚焦，拒绝牵引系统鼠标".to_string());
    }
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let size = window.inner_size().map_err(|e| e.to_string())?;
    let max_x = f64::from(size.width) / scale;
    let max_y = f64::from(size.height) / scale;
    let bounded_x = x.clamp(0.0, max_x);
    let bounded_y = y.clamp(0.0, max_y);
    let px = (bounded_x * scale).round() as i32;
    let py = (bounded_y * scale).round() as i32;
    let result = window
        .set_cursor_position(tauri::PhysicalPosition::new(px, py))
        .map_err(|e| e.to_string());
    let n = LOG_COUNT.fetch_add(1, Ordering::Relaxed);
    if n < 3 {
        tracing::info!(
            "[warp_cursor] logical=({bounded_x:.0},{bounded_y:.0}) scale={scale} client=({px},{py}) ok={}",
            result.is_ok()
        );
    }
    if let Err(ref e) = result {
        tracing::warn!("[warp_cursor] 设置光标位置失败: {e}");
    }
    result
}

/// Esc、失焦、隐藏或 5 秒时限到达后只停止移动鼠标，不替玩家选择。
#[tauri::command]
pub async fn cancel_script_cursor_warp(
    window: tauri::WebviewWindow,
    request_id: String,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("只有主窗口可以取消 force_choice 鼠标牵引".to_string());
    }
    let state = window.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    let Some(guard) = channels
        .extension_mut::<NativeChannels>()
        .force_choice_guard
        .as_mut()
    else {
        return Ok(());
    };
    if guard.request_id != request_id {
        return Err("force_choice 鼠标票据无效或已过期".to_string());
    }
    guard.warp_enabled = false;
    Ok(())
}

#[tauri::command]
pub async fn script_submit_poem(
    app: AppHandle,
    request_id: String,
    result: serde_json::Value,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut channels = state.script_channels.lock().await;
    let pending = channels
        .extension_mut::<NativeChannels>()
        .poem_tx
        .as_ref()
        .ok_or("当前没有等待写诗结果的脚本事件")?;
    if pending.request_id != request_id {
        return Err("写诗提交票据无效或已过期".to_string());
    }
    let raw = serde_json::to_string(&result).map_err(|error| format!("写诗结果无效: {error}"))?;
    let pending = channels
        .extension_mut::<NativeChannels>()
        .poem_tx
        .take()
        .expect("poem_tx checked above");
    pending
        .tx
        .send(raw)
        .map_err(|_| "写诗互动已结束".to_string())
}
