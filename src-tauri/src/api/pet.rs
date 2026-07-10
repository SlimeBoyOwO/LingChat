use serde::Deserialize;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
#[cfg(desktop)]
use tauri::{PhysicalPosition, PhysicalSize};

#[cfg(desktop)]
use crate::window_geometry::{WindowDimensions, WindowSizePlan};
use crate::{config, window_geometry};

#[cfg(desktop)]
const PET_BASE_WIDTH: f64 = 240.0;
#[cfg(desktop)]
const PET_AVATAR_HEIGHT: f64 = 240.0;
#[cfg(desktop)]
const PET_DIALOG_HEIGHT: f64 = 75.0;
#[cfg(desktop)]
const PET_CHAT_HEIGHT: f64 = 45.0;
#[cfg(desktop)]
const MIN_PET_SCALE: f64 = 0.7;
#[cfg(desktop)]
const MAX_PET_SCALE: f64 = 1.3;

#[cfg(desktop)]
fn window_operation_error(operation: &str, error: impl std::fmt::Display) -> String {
    let message = format!("桌宠窗口操作失败（{operation}）: {error}");
    tracing::error!("{message}");
    message
}

#[cfg(desktop)]
fn pet_window_size(scale: Option<f64>) -> Result<WindowDimensions, String> {
    let scale = scale.unwrap_or(1.0);
    if !scale.is_finite() || !(MIN_PET_SCALE..=MAX_PET_SCALE).contains(&scale) {
        let message =
            format!("无效的桌宠缩放比例 {scale}，允许范围为 {MIN_PET_SCALE}..={MAX_PET_SCALE}");
        tracing::warn!("{message}");
        return Err(message);
    }

    // 与前端 calcWindowLayout 保持一致，避免 CSS 布局与窗口逻辑尺寸相差 1px。
    let width = (PET_BASE_WIDTH * scale).round();
    let height = (PET_AVATAR_HEIGHT * scale).round()
        + (PET_DIALOG_HEIGHT * scale).round()
        + (PET_CHAT_HEIGHT * scale).round();
    if !width.is_finite() || !height.is_finite() || width < 1.0 || height < 1.0 {
        let message = format!("桌宠目标窗口尺寸无效: {width}x{height} (scale={scale})");
        tracing::warn!("{message}");
        return Err(message);
    }

    Ok(WindowDimensions::new(width as u32, height as u32))
}

#[cfg(desktop)]
fn configured_window_plan(
    app: &AppHandle,
    window: &tauri::WebviewWindow<tauri::Wry>,
) -> Result<(WindowSizePlan, Option<&'static str>), String> {
    let width_raw = config::read_setting(
        app,
        config::keys::WINDOW_WIDTH,
        &window_geometry::MAIN_WINDOW_DEFAULT_WIDTH.to_string(),
    );
    let height_raw = config::read_setting(
        app,
        config::keys::WINDOW_HEIGHT,
        &window_geometry::MAIN_WINDOW_DEFAULT_HEIGHT.to_string(),
    );
    let (requested, repair_preset) =
        match window_geometry::parse_main_window_size(&width_raw, &height_raw) {
            Ok(size) => (size, None),
            Err(error) => {
                tracing::warn!("退出桌宠时发现主窗口尺寸配置无效，将恢复安全默认值：{error}");
                (window_geometry::default_main_window_size(), Some("default"))
            }
        };
    let plan = window_geometry::plan_main_window_size(window, requested)?;
    Ok((plan, repair_preset))
}

#[cfg(desktop)]
#[derive(Clone, Debug)]
struct NormalWindowSnapshot {
    inner_size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    fullscreen: bool,
    maximized: bool,
    maximizable: bool,
}

#[cfg(desktop)]
fn capture_normal_window_snapshot(
    window: &tauri::WebviewWindow<tauri::Wry>,
) -> Result<NormalWindowSnapshot, String> {
    let fullscreen = window
        .is_fullscreen()
        .map_err(|error| window_operation_error("读取全屏状态", error))?;
    let maximized = window
        .is_maximized()
        .map_err(|error| window_operation_error("读取最大化状态", error))?;
    let maximizable = window
        .is_maximizable()
        .map_err(|error| window_operation_error("读取可最大化状态", error))?;

    let leave_result = (|| -> Result<(), String> {
        if fullscreen {
            window
                .set_fullscreen(false)
                .map_err(|error| window_operation_error("退出全屏", error))?;
        }
        if maximized {
            window
                .unmaximize()
                .map_err(|error| window_operation_error("取消最大化", error))?;
        }
        Ok(())
    })();

    if let Err(error) = leave_result {
        let mut rollback_errors = Vec::new();
        if maximized {
            record_window_result(&mut rollback_errors, "恢复最大化状态", window.maximize());
        }
        if fullscreen {
            record_window_result(
                &mut rollback_errors,
                "恢复全屏状态",
                window.set_fullscreen(true),
            );
        }
        return Err(if rollback_errors.is_empty() {
            error.clone()
        } else {
            format!(
                "{error}；窗口状态回滚不完整：{}",
                rollback_errors.join("；")
            )
        });
    }

    let snapshot = (|| -> Result<NormalWindowSnapshot, String> {
        Ok(NormalWindowSnapshot {
            inner_size: window
                .inner_size()
                .map_err(|error| window_operation_error("读取普通主窗口尺寸", error))?,
            position: window
                .outer_position()
                .map_err(|error| window_operation_error("读取普通主窗口位置", error))?,
            fullscreen,
            maximized,
            maximizable,
        })
    })();

    if let Err(error) = snapshot.as_ref() {
        let mut rollback_errors = Vec::new();
        if maximized {
            record_window_result(&mut rollback_errors, "恢复最大化状态", window.maximize());
        }
        if fullscreen {
            record_window_result(
                &mut rollback_errors,
                "恢复全屏状态",
                window.set_fullscreen(true),
            );
        }
        return Err(if rollback_errors.is_empty() {
            error.clone()
        } else {
            format!(
                "{error}；窗口状态回滚不完整：{}",
                rollback_errors.join("；")
            )
        });
    }

    snapshot
}

#[cfg(desktop)]
fn record_window_result(errors: &mut Vec<String>, operation: &str, result: tauri::Result<()>) {
    if let Err(error) = result {
        errors.push(window_operation_error(operation, error));
    }
}

#[cfg(desktop)]
fn restore_snapshot_best_effort(
    window: &tauri::WebviewWindow<tauri::Wry>,
    snapshot: &NormalWindowSnapshot,
) -> Vec<String> {
    let mut errors = Vec::new();

    record_window_result(
        &mut errors,
        "恢复鼠标事件",
        window.set_ignore_cursor_events(false),
    );
    record_window_result(
        &mut errors,
        "恢复任务栏图标",
        window.set_skip_taskbar(false),
    );
    record_window_result(&mut errors, "取消窗口置顶", window.set_always_on_top(false));
    record_window_result(&mut errors, "恢复尺寸调整", window.set_resizable(true));
    record_window_result(
        &mut errors,
        "恢复可最大化状态",
        window.set_maximizable(snapshot.maximizable),
    );
    record_window_result(&mut errors, "恢复窗口装饰", window.set_decorations(true));
    if let Err(error) = window_geometry::set_main_window_constraints(window) {
        errors.push(error);
    }
    record_window_result(
        &mut errors,
        "恢复原主窗口尺寸",
        window.set_size(snapshot.inner_size),
    );
    record_window_result(
        &mut errors,
        "恢复原主窗口位置",
        window.set_position(snapshot.position),
    );
    if snapshot.maximized {
        record_window_result(&mut errors, "恢复最大化状态", window.maximize());
    }
    if snapshot.fullscreen {
        record_window_result(&mut errors, "恢复全屏状态", window.set_fullscreen(true));
    }
    errors
}

#[derive(Clone, Deserialize, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct HitTestState {
    pub solid_rects: Arc<Mutex<Vec<Rect>>>,
    pub enabled: Arc<Mutex<bool>>,
    pub transition_lock: Arc<Mutex<()>>,
    #[cfg(desktop)]
    normal_window: Arc<Mutex<Option<NormalWindowSnapshot>>>,
}

impl Default for HitTestState {
    fn default() -> Self {
        Self {
            solid_rects: Arc::new(Mutex::new(Vec::new())),
            enabled: Arc::new(Mutex::new(false)),
            transition_lock: Arc::new(Mutex::new(())),
            #[cfg(desktop)]
            normal_window: Arc::new(Mutex::new(None)),
        }
    }
}

#[tauri::command]
pub fn update_solid_regions(rects: Vec<Rect>, state: tauri::State<'_, HitTestState>) {
    if let Ok(mut locked) = state.solid_rects.lock() {
        *locked = rects;
    }
}

#[tauri::command]
pub fn set_pet_mode(
    enable: bool,
    scale: Option<f64>,
    app_handle: AppHandle,
    state: tauri::State<'_, HitTestState>,
) -> Result<(), String> {
    // Serialize native mode changes and resolution saves without holding the
    // small state mutex across the whole sequence of GUI operations.
    let _transition_guard = state
        .transition_lock
        .lock()
        .map_err(|error| format!("等待窗口模式切换失败：{error}"))?;

    #[cfg(desktop)]
    {
        let window = app_handle.get_webview_window("main").ok_or_else(|| {
            let message = "找不到 main 窗口，无法切换桌宠模式".to_string();
            tracing::error!("{message}");
            message
        })?;
        let currently_enabled = *state
            .enabled
            .lock()
            .map_err(|error| format!("读取桌宠模式状态失败：{error}"))?;

        if enable {
            let target_size = pet_window_size(scale)?;
            // Clear any stale click-through state before leaving fullscreen or
            // changing styles so a failed transition remains recoverable.
            window
                .set_ignore_cursor_events(false)
                .map_err(|error| window_operation_error("重置鼠标事件", error))?;
            let snapshot = if currently_enabled {
                None
            } else {
                Some(capture_normal_window_snapshot(&window)?)
            };

            let enter_result = (|| -> Result<(), String> {
                if snapshot.is_some() {
                    window_geometry::clear_window_minimum_size(&window)?;
                    window
                        .set_skip_taskbar(true)
                        .map_err(|error| window_operation_error("隐藏任务栏图标", error))?;
                    window
                        .set_always_on_top(true)
                        .map_err(|error| window_operation_error("设置窗口置顶", error))?;
                    window
                        .set_resizable(false)
                        .map_err(|error| window_operation_error("禁止调整尺寸", error))?;
                    window
                        .set_maximizable(false)
                        .map_err(|error| window_operation_error("禁止窗口最大化", error))?;
                    window
                        .set_decorations(false)
                        .map_err(|error| window_operation_error("隐藏窗口装饰", error))?;
                } else {
                    // Scale changes while already in pet mode must not inherit
                    // a restored main-window minimum constraint.
                    window_geometry::clear_window_minimum_size(&window)?;
                }

                let plan = window_geometry::plan_borderless_window_size(
                    &window,
                    target_size,
                    WindowDimensions::new(1, 1),
                )?;
                window_geometry::apply_window_size_plan(
                    &window,
                    &plan,
                    snapshot.as_ref().map(|snapshot| snapshot.position),
                )?;
                tracing::info!(
                    width = plan.applied.width,
                    height = plan.applied.height,
                    mode_changed = !currently_enabled,
                    adjusted = plan.adjusted,
                    "桌宠窗口尺寸与原生状态已应用"
                );
                Ok(())
            })();

            if let Err(error) = enter_result {
                if let Some(snapshot) = snapshot.as_ref() {
                    let rollback_errors = restore_snapshot_best_effort(&window, snapshot);
                    if !rollback_errors.is_empty() {
                        tracing::error!(
                            "进入桌宠失败后的窗口回滚不完整：{}",
                            rollback_errors.join("；")
                        );
                    }
                    if let Ok(mut enabled) = state.enabled.lock() {
                        *enabled = false;
                    }
                }
                return Err(error);
            }

            if let Some(snapshot) = snapshot {
                let commit_result = (|| -> Result<(), String> {
                    *state
                        .normal_window
                        .lock()
                        .map_err(|error| format!("保存主窗口状态失败：{error}"))? =
                        Some(snapshot.clone());
                    *state
                        .enabled
                        .lock()
                        .map_err(|error| format!("提交桌宠模式状态失败：{error}"))? = true;
                    Ok(())
                })();
                if let Err(error) = commit_result {
                    let rollback_errors = restore_snapshot_best_effort(&window, &snapshot);
                    if let Ok(mut normal_window) = state.normal_window.lock() {
                        *normal_window = None;
                    }
                    if let Ok(mut enabled) = state.enabled.lock() {
                        *enabled = false;
                    }
                    return Err(if rollback_errors.is_empty() {
                        error
                    } else {
                        format!("{error}；窗口回滚不完整：{}", rollback_errors.join("；"))
                    });
                }
            } else {
                *state
                    .enabled
                    .lock()
                    .map_err(|error| format!("提交桌宠模式状态失败：{error}"))? = true;
            }
        } else {
            // Disable hit testing immediately.  Even if a later cosmetic or
            // positioning operation fails, the user must never be left with a
            // click-through normal window.
            *state
                .enabled
                .lock()
                .map_err(|error| format!("更新桌宠模式状态失败：{error}"))? = false;

            let snapshot = state
                .normal_window
                .lock()
                .map_err(|error| format!("读取主窗口快照失败：{error}"))?
                .clone();
            let mut errors = Vec::new();

            record_window_result(
                &mut errors,
                "恢复鼠标事件",
                window.set_ignore_cursor_events(false),
            );
            record_window_result(
                &mut errors,
                "恢复任务栏图标",
                window.set_skip_taskbar(false),
            );
            record_window_result(&mut errors, "取消窗口置顶", window.set_always_on_top(false));
            record_window_result(&mut errors, "恢复尺寸调整", window.set_resizable(true));
            record_window_result(
                &mut errors,
                "恢复可最大化状态",
                window.set_maximizable(
                    snapshot
                        .as_ref()
                        .map(|snapshot| snapshot.maximizable)
                        .unwrap_or(true),
                ),
            );
            record_window_result(&mut errors, "恢复窗口装饰", window.set_decorations(true));

            match configured_window_plan(&app_handle, &window) {
                Ok((plan, repair_preset)) => {
                    match window_geometry::apply_main_window_plan(&window, &plan, None) {
                        Err(error) => errors.push(error),
                        Ok(applied) => {
                            let final_plan = applied.plan;
                            if repair_preset.is_some() || final_plan.adjusted {
                                if let Err(error) = config::persist_main_window_size(
                                    &app_handle,
                                    final_plan.applied,
                                    repair_preset,
                                ) {
                                    tracing::warn!(
                                        "主窗口已安全恢复，但修复持久化配置失败：{error}"
                                    );
                                }
                            }
                            tracing::info!(
                                width = final_plan.applied.width,
                                height = final_plan.applied.height,
                                adjusted = final_plan.adjusted,
                                "已恢复安全主窗口尺寸"
                            );
                        }
                    }
                }
                Err(error) => errors.push(error),
            }

            if let Some(snapshot) = snapshot.as_ref() {
                if snapshot.maximized {
                    record_window_result(&mut errors, "恢复最大化状态", window.maximize());
                }
                if snapshot.fullscreen {
                    record_window_result(&mut errors, "恢复全屏状态", window.set_fullscreen(true));
                }
            }

            if errors.is_empty() {
                *state
                    .normal_window
                    .lock()
                    .map_err(|error| format!("清理主窗口快照失败：{error}"))? = None;
            } else {
                let message = format!("恢复主窗口时发生错误：{}", errors.join("；"));
                tracing::error!("{message}");
                return Err(message);
            }
        }

        tracing::info!(enable, "桌宠窗口模式已应用");
    }

    #[cfg(not(desktop))]
    {
        *state
            .enabled
            .lock()
            .map_err(|error| format!("更新桌宠模式状态失败：{error}"))? = enable;
    }

    Ok(())
}
