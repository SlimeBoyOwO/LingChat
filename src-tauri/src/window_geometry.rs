use serde::Serialize;
use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, WebviewWindow, Wry};

pub const MAIN_WINDOW_DEFAULT_WIDTH: u32 = 1500;
pub const MAIN_WINDOW_DEFAULT_HEIGHT: u32 = 800;
pub const MAIN_WINDOW_MIN_WIDTH: u32 = 1024;
pub const MAIN_WINDOW_MIN_HEIGHT: u32 = 640;
const MAX_DIMENSION: u32 = 16_384;

// Keep a small safety gap inside the operating system work area.  Main-window
// frame fallbacks are logical values and are converted per monitor so mixed-DPI
// displays do not under-reserve title-bar space.
const EDGE_MARGIN_PHYSICAL: u32 = 8;
const MIN_MAIN_FRAME_WIDTH_LOGICAL: f64 = 8.0;
const MIN_MAIN_FRAME_HEIGHT_LOGICAL: f64 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowDimensions {
    pub width: u32,
    pub height: u32,
}

impl WindowDimensions {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn logical_size(self) -> LogicalSize<f64> {
        LogicalSize::new(self.width as f64, self.height as f64)
    }
}

#[derive(Clone, Copy, Debug)]
struct WorkAreaMetrics {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale_factor: f64,
    frame_width: u32,
    frame_height: u32,
}

#[derive(Clone, Copy, Debug)]
enum FramePolicy {
    CurrentWindow,
    DecoratedMainWindow,
    BorderlessWindow,
}

#[derive(Clone, Copy, Debug)]
pub struct WindowSizePlan {
    pub requested: WindowDimensions,
    pub applied: WindowDimensions,
    pub adjusted: bool,
    metrics: Option<WorkAreaMetrics>,
}

pub const fn default_main_window_size() -> WindowDimensions {
    WindowDimensions::new(MAIN_WINDOW_DEFAULT_WIDTH, MAIN_WINDOW_DEFAULT_HEIGHT)
}

fn parse_dimension(raw: &str, label: &str, minimum: u32) -> Result<u32, String> {
    let trimmed = raw.trim();
    let parsed = trimmed
        .parse::<u32>()
        .map_err(|error| format!("{label}无效（{trimmed}）：请输入整数，错误为 {error}"))?;

    if !(minimum..=MAX_DIMENSION).contains(&parsed) {
        return Err(format!(
            "{label}超出允许范围：{parsed}（允许 {minimum}..={MAX_DIMENSION}，并会进一步受当前显示器工作区限制）"
        ));
    }

    Ok(parsed)
}

pub fn parse_main_window_size(
    width_raw: &str,
    height_raw: &str,
) -> Result<WindowDimensions, String> {
    Ok(WindowDimensions::new(
        parse_dimension(width_raw, "主窗口宽度", MAIN_WINDOW_MIN_WIDTH)?,
        parse_dimension(height_raw, "主窗口高度", MAIN_WINDOW_MIN_HEIGHT)?,
    ))
}

fn work_area_metrics(
    window: &WebviewWindow<Wry>,
    frame_policy: FramePolicy,
) -> Result<Option<WorkAreaMetrics>, String> {
    let monitor = match window.current_monitor() {
        Ok(Some(monitor)) => Some(monitor),
        Ok(None) => window
            .primary_monitor()
            .map_err(|error| format!("无法读取主显示器信息以校验窗口尺寸：{error}"))?,
        Err(current_error) => {
            tracing::warn!("读取当前显示器失败，将尝试主显示器：{current_error}");
            window.primary_monitor().map_err(|primary_error| {
                format!(
                    "无法读取显示器信息以校验窗口尺寸：当前显示器错误 {current_error}；主显示器错误 {primary_error}"
                )
            })?
        }
    };

    let Some(monitor) = monitor else {
        return Ok(None);
    };

    let scale_factor = monitor.scale_factor();
    if !scale_factor.is_finite() || scale_factor <= 0.0 {
        return Err(format!("显示器缩放比例无效：{scale_factor}"));
    }

    let work_area = monitor.work_area();
    let (measured_frame_width, measured_frame_height) =
        match (window.inner_size(), window.outer_size()) {
            (Ok(inner), Ok(outer)) => (
                outer.width.saturating_sub(inner.width),
                outer.height.saturating_sub(inner.height),
            ),
            _ => (0, 0),
        };
    let currently_decorated = window.is_decorated().unwrap_or_else(|error| {
        tracing::warn!("读取窗口装饰状态失败，将按有边框窗口预留空间：{error}");
        true
    });
    let (frame_width, frame_height) = match frame_policy {
        FramePolicy::BorderlessWindow => (0, 0),
        FramePolicy::DecoratedMainWindow => (
            measured_frame_width.max((MIN_MAIN_FRAME_WIDTH_LOGICAL * scale_factor).ceil() as u32),
            measured_frame_height.max((MIN_MAIN_FRAME_HEIGHT_LOGICAL * scale_factor).ceil() as u32),
        ),
        FramePolicy::CurrentWindow if currently_decorated => (
            measured_frame_width.max((MIN_MAIN_FRAME_WIDTH_LOGICAL * scale_factor).ceil() as u32),
            measured_frame_height.max((MIN_MAIN_FRAME_HEIGHT_LOGICAL * scale_factor).ceil() as u32),
        ),
        FramePolicy::CurrentWindow => (measured_frame_width, measured_frame_height),
    };

    Ok(Some(WorkAreaMetrics {
        x: work_area.position.x,
        y: work_area.position.y,
        width: work_area.size.width,
        height: work_area.size.height,
        scale_factor,
        frame_width,
        frame_height,
    }))
}

fn maximum_logical_inner_size(metrics: WorkAreaMetrics) -> WindowDimensions {
    let horizontal_reserve = metrics
        .frame_width
        .saturating_add(EDGE_MARGIN_PHYSICAL.saturating_mul(2));
    let vertical_reserve = metrics
        .frame_height
        .saturating_add(EDGE_MARGIN_PHYSICAL.saturating_mul(2));

    WindowDimensions::new(
        (metrics.width.saturating_sub(horizontal_reserve) as f64 / metrics.scale_factor)
            .floor()
            .max(1.0) as u32,
        (metrics.height.saturating_sub(vertical_reserve) as f64 / metrics.scale_factor)
            .floor()
            .max(1.0) as u32,
    )
}

fn clamp_dimensions_to_work_area(
    requested: WindowDimensions,
    maximum: WindowDimensions,
) -> WindowDimensions {
    WindowDimensions::new(
        requested.width.min(maximum.width),
        requested.height.min(maximum.height),
    )
}

fn plan_window_size_with_policy(
    window: &WebviewWindow<Wry>,
    requested: WindowDimensions,
    minimum: WindowDimensions,
    frame_policy: FramePolicy,
) -> Result<WindowSizePlan, String> {
    if requested.width < minimum.width || requested.height < minimum.height {
        return Err(format!(
            "窗口目标尺寸 {}x{} 低于界面可用下限 {}x{}",
            requested.width, requested.height, minimum.width, minimum.height
        ));
    }

    let metrics = work_area_metrics(window, frame_policy)?;
    let applied = if let Some(metrics) = metrics {
        let maximum = maximum_logical_inner_size(metrics);
        if maximum.width < minimum.width || maximum.height < minimum.height {
            return Err(format!(
                "当前显示器可用工作区过小：最多约 {}x{} 逻辑像素，应用至少需要 {}x{}",
                maximum.width, maximum.height, minimum.width, minimum.height
            ));
        }
        clamp_dimensions_to_work_area(requested, maximum)
    } else {
        requested
    };

    if applied.width < minimum.width || applied.height < minimum.height {
        return Err(format!(
            "目标尺寸 {}x{} 适配当前显示器后仅剩 {}x{}，低于界面可用下限 {}x{}",
            requested.width,
            requested.height,
            applied.width,
            applied.height,
            minimum.width,
            minimum.height
        ));
    }

    Ok(WindowSizePlan {
        requested,
        applied,
        adjusted: requested != applied,
        metrics,
    })
}

pub fn plan_borderless_window_size(
    window: &WebviewWindow<Wry>,
    requested: WindowDimensions,
    minimum: WindowDimensions,
) -> Result<WindowSizePlan, String> {
    plan_window_size_with_policy(window, requested, minimum, FramePolicy::BorderlessWindow)
}

pub fn plan_main_window_size(
    window: &WebviewWindow<Wry>,
    requested: WindowDimensions,
) -> Result<WindowSizePlan, String> {
    plan_window_size_with_policy(
        window,
        requested,
        WindowDimensions::new(MAIN_WINDOW_MIN_WIDTH, MAIN_WINDOW_MIN_HEIGHT),
        FramePolicy::DecoratedMainWindow,
    )
}

pub fn recommended_main_window_size(
    window: &WebviewWindow<Wry>,
) -> Result<WindowDimensions, String> {
    let Some(metrics) = work_area_metrics(window, FramePolicy::DecoratedMainWindow)? else {
        return Ok(default_main_window_size());
    };

    let maximum = maximum_logical_inner_size(metrics);
    let box_width = (maximum.width as f64 * 0.9).floor() as u32;
    let box_height = (maximum.height as f64 * 0.9).floor() as u32;
    let default_aspect = MAIN_WINDOW_DEFAULT_WIDTH as f64 / MAIN_WINDOW_DEFAULT_HEIGHT as f64;

    let (width, height) = if box_width as f64 / box_height as f64 > default_aspect {
        (
            (box_height as f64 * default_aspect).floor() as u32,
            box_height,
        )
    } else {
        (
            box_width,
            (box_width as f64 / default_aspect).floor() as u32,
        )
    };

    let requested = WindowDimensions::new(
        width.max(MAIN_WINDOW_MIN_WIDTH),
        height.max(MAIN_WINDOW_MIN_HEIGHT),
    );
    Ok(plan_main_window_size(window, requested)?.applied)
}

fn clamp_i64(value: i64, minimum: i64, maximum: i64) -> i64 {
    value.max(minimum).min(maximum.max(minimum))
}

pub fn apply_window_size_plan(
    window: &WebviewWindow<Wry>,
    plan: &WindowSizePlan,
    preferred_position: Option<PhysicalPosition<i32>>,
) -> Result<(), String> {
    window
        .set_size(plan.applied.logical_size())
        .map_err(|error| format!("调整窗口内容区尺寸失败：{error}"))?;

    // Re-read the monitor and real outer size after resizing.  The window may
    // have crossed into a display with another DPI, and estimates made before
    // set_size are not authoritative for safe positioning.
    let metrics = work_area_metrics(window, FramePolicy::CurrentWindow)?.or(plan.metrics);
    let Some(metrics) = metrics else {
        return Ok(());
    };

    let current_position = preferred_position
        .or_else(|| window.outer_position().ok())
        .unwrap_or_else(|| PhysicalPosition::new(metrics.x, metrics.y));

    let actual_outer_size = window.outer_size().ok();
    let outer_width = actual_outer_size.map(|size| size.width).unwrap_or_else(|| {
        (plan.applied.width as f64 * metrics.scale_factor).round() as u32 + metrics.frame_width
    });
    let outer_height = actual_outer_size
        .map(|size| size.height)
        .unwrap_or_else(|| {
            (plan.applied.height as f64 * metrics.scale_factor).round() as u32
                + metrics.frame_height
        });

    let min_x = metrics.x as i64 + EDGE_MARGIN_PHYSICAL as i64;
    let min_y = metrics.y as i64 + EDGE_MARGIN_PHYSICAL as i64;
    let max_x =
        metrics.x as i64 + metrics.width as i64 - outer_width as i64 - EDGE_MARGIN_PHYSICAL as i64;
    let max_y = metrics.y as i64 + metrics.height as i64
        - outer_height as i64
        - EDGE_MARGIN_PHYSICAL as i64;
    let safe_position = PhysicalPosition::new(
        clamp_i64(current_position.x as i64, min_x, max_x) as i32,
        clamp_i64(current_position.y as i64, min_y, max_y) as i32,
    );

    if safe_position != current_position {
        window.set_position(safe_position).map_err(|error| {
            format!(
                "窗口尺寸已调整，但无法将位置安全限制到 ({}, {})：{error}",
                safe_position.x, safe_position.y
            )
        })?;
    }

    Ok(())
}

pub fn set_main_window_constraints(window: &WebviewWindow<Wry>) -> Result<(), String> {
    window
        .set_min_size(Some(LogicalSize::new(
            MAIN_WINDOW_MIN_WIDTH as f64,
            MAIN_WINDOW_MIN_HEIGHT as f64,
        )))
        .map_err(|error| format!("设置主窗口最小尺寸失败：{error}"))
}

pub fn clear_window_minimum_size(window: &WebviewWindow<Wry>) -> Result<(), String> {
    window
        .set_min_size(None::<LogicalSize<f64>>)
        .map_err(|error| format!("解除主窗口最小尺寸失败：{error}"))
}

#[derive(Clone, Debug)]
struct MainWindowRollbackSnapshot {
    inner_size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    fullscreen: bool,
    maximized: bool,
}

#[derive(Clone, Debug)]
pub struct AppliedMainWindowPlan {
    pub plan: WindowSizePlan,
    rollback: MainWindowRollbackSnapshot,
}

fn restore_main_window_snapshot(
    window: &WebviewWindow<Wry>,
    snapshot: &MainWindowRollbackSnapshot,
) -> Result<(), String> {
    let mut errors = Vec::new();

    if let Err(error) = set_main_window_constraints(window) {
        errors.push(error);
    }
    if let Err(error) = window.set_size(snapshot.inner_size) {
        errors.push(format!("恢复调整前的窗口尺寸失败：{error}"));
    }
    if let Err(error) = window.set_position(snapshot.position) {
        errors.push(format!("恢复调整前的窗口位置失败：{error}"));
    }
    if snapshot.maximized {
        if let Err(error) = window.maximize() {
            errors.push(format!("恢复调整前的最大化状态失败：{error}"));
        }
    }
    if snapshot.fullscreen {
        if let Err(error) = window.set_fullscreen(true) {
            errors.push(format!("恢复调整前的全屏状态失败：{error}"));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

fn leave_display_state_and_capture_normal_bounds(
    window: &WebviewWindow<Wry>,
) -> Result<MainWindowRollbackSnapshot, String> {
    let fullscreen = window
        .is_fullscreen()
        .map_err(|error| format!("读取调整前的全屏状态失败：{error}"))?;
    let maximized = window
        .is_maximized()
        .map_err(|error| format!("读取调整前的最大化状态失败：{error}"))?;

    let leave_result = (|| -> Result<(), String> {
        if fullscreen {
            window
                .set_fullscreen(false)
                .map_err(|error| format!("退出全屏以应用窗口尺寸失败：{error}"))?;
        }
        if maximized {
            window
                .unmaximize()
                .map_err(|error| format!("取消最大化以应用窗口尺寸失败：{error}"))?;
        }
        Ok(())
    })();

    if let Err(error) = leave_result {
        let mut rollback_errors = Vec::new();
        if maximized {
            if let Err(rollback_error) = window.maximize() {
                rollback_errors.push(format!("恢复最大化状态失败：{rollback_error}"));
            }
        }
        if fullscreen {
            if let Err(rollback_error) = window.set_fullscreen(true) {
                rollback_errors.push(format!("恢复全屏状态失败：{rollback_error}"));
            }
        }
        return Err(if rollback_errors.is_empty() {
            error
        } else {
            format!(
                "{error}；窗口状态回滚不完整：{}",
                rollback_errors.join("；")
            )
        });
    }

    let normal_bounds = (|| -> Result<MainWindowRollbackSnapshot, String> {
        Ok(MainWindowRollbackSnapshot {
            inner_size: window
                .inner_size()
                .map_err(|error| format!("读取普通窗口恢复尺寸失败：{error}"))?,
            position: window
                .outer_position()
                .map_err(|error| format!("读取普通窗口恢复位置失败：{error}"))?,
            fullscreen,
            maximized,
        })
    })();

    if normal_bounds.is_err() {
        let mut state_errors = Vec::new();
        if maximized {
            if let Err(error) = window.maximize() {
                state_errors.push(format!("恢复最大化状态失败：{error}"));
            }
        }
        if fullscreen {
            if let Err(error) = window.set_fullscreen(true) {
                state_errors.push(format!("恢复全屏状态失败：{error}"));
            }
        }
        if !state_errors.is_empty() {
            tracing::error!(
                "读取普通窗口边界失败后的状态回滚不完整：{}",
                state_errors.join("；")
            );
        }
    }

    normal_bounds
}

pub fn apply_main_window_plan(
    window: &WebviewWindow<Wry>,
    plan: &WindowSizePlan,
    preferred_position: Option<PhysicalPosition<i32>>,
) -> Result<AppliedMainWindowPlan, String> {
    // Capture normal restore bounds only after leaving fullscreen/maximized;
    // those states otherwise report screen-sized geometry instead of the real
    // normal-window bounds.
    let rollback = leave_display_state_and_capture_normal_bounds(window)?;

    let apply_result = (|| -> Result<WindowSizePlan, String> {
        set_main_window_constraints(window)?;
        // Re-plan after leaving fullscreen/maximized so frame, monitor and DPI
        // measurements describe the state that will actually be resized.
        let final_plan = plan_main_window_size(window, plan.requested)?;
        apply_window_size_plan(window, &final_plan, preferred_position)?;
        Ok(final_plan)
    })();

    match apply_result {
        Ok(final_plan) => Ok(AppliedMainWindowPlan {
            plan: final_plan,
            rollback,
        }),
        Err(error) => match restore_main_window_snapshot(window, &rollback) {
            Ok(()) => Err(error),
            Err(rollback_error) => Err(format!("{error}；窗口状态回滚不完整：{rollback_error}")),
        },
    }
}

pub fn rollback_applied_main_window_plan(
    window: &WebviewWindow<Wry>,
    applied: &AppliedMainWindowPlan,
) -> Result<(), String> {
    restore_main_window_snapshot(window, &applied.rollback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_integer_main_window_dimensions() {
        assert_eq!(
            parse_main_window_size("1500", "800").unwrap(),
            WindowDimensions::new(1500, 800)
        );
        assert!(parse_main_window_size("1500.5", "800").is_err());
        assert!(parse_main_window_size("1023", "800").is_err());
        assert!(parse_main_window_size("1500", "639").is_err());
    }

    #[test]
    fn clamping_uses_each_available_axis_independently() {
        assert_eq!(
            clamp_dimensions_to_work_area(
                WindowDimensions::new(2560, 1440),
                WindowDimensions::new(1900, 960),
            ),
            WindowDimensions::new(1900, 960)
        );
        assert_eq!(
            clamp_dimensions_to_work_area(
                WindowDimensions::new(1600, 640),
                WindowDimensions::new(1500, 900),
            ),
            WindowDimensions::new(1500, 640)
        );
    }

    #[test]
    fn fitting_keeps_dimensions_that_already_fit() {
        let requested = WindowDimensions::new(1500, 800);
        assert_eq!(
            clamp_dimensions_to_work_area(requested, WindowDimensions::new(1900, 960)),
            requested
        );
    }
}
