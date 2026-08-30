use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
#[cfg(desktop)]
use tauri::LogicalSize;
use tauri::AppHandle;
#[cfg(desktop)]
use tauri::Manager;

// 桌宠点击穿透命中区（桌面端专属，移动端仅作为命令参数反序列化、不读取）。
#[cfg_attr(not(desktop), allow(dead_code))]
#[derive(Clone, Deserialize, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 桌宠窗口内的鼠标位置（逻辑/CSS 像素），由 Rust 侧全局轮询循环计算并广播给前端。
/// 坐标系与 DOM 的 clientX/clientY 一致（窗口非装饰时即 webview 视口坐标）。
#[cfg_attr(not(desktop), allow(dead_code))]
#[derive(Clone, Debug, Serialize)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

pub struct HitTestState {
    pub solid_rects: Arc<Mutex<Vec<Rect>>>,
    pub enabled: Arc<Mutex<bool>>,
}

impl Default for HitTestState {
    fn default() -> Self {
        Self {
            solid_rects: Arc::new(Mutex::new(Vec::new())),
            enabled: Arc::new(Mutex::new(false)),
        }
    }
}

#[tauri::command]
pub fn update_solid_regions(rects: Vec<Rect>, state: tauri::State<'_, HitTestState>) {
    if let Ok(mut locked) = state.solid_rects.lock() {
        *locked = rects;
    }
}

/// 退出全屏，并等到它真正结束。
///
/// 窗口处于全屏时，平台会吞掉后续的 `set_decorations` / `set_size`：
/// tao 在 macOS 上是 `if fullscreen { return; }` 直接跳过，只把 decorations
/// 记进状态；等真正退出全屏时，`restore_state_from_fullscreen` 又用的是
/// **进入全屏那一刻保存的** style mask，中途记下的状态就丢了。Windows 侧
/// 机制不同（改 `MARKER_DECORATIONS` 标志位），但同样会和全屏标志打架。
/// 结果就是全屏状态下进出桌宠模式，窗口边框会消失。
#[cfg(desktop)]
async fn leave_fullscreen(window: &tauri::WebviewWindow) {
    if !window.is_fullscreen().unwrap_or(false) {
        return;
    }
    let _ = window.set_fullscreen(false);

    // macOS 退出全屏带动画，动画期间 is_fullscreen() 仍然是 true，
    // 这时改窗口一样会被丢掉，所以要等到状态真的翻过来。
    // 超时就继续往下走：降级成修复前的行为，总好过让桌宠模式进不去。
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if !window.is_fullscreen().unwrap_or(false) {
            return;
        }
    }
}

/// 把主窗口恢复为标准尺寸并居中，且保证不超出当前显示器工作区（不盖任务栏）。
///
/// 用物理像素 + 工作区矩形计算，避免 LogicalSize 依赖窗口当前 DPI 造成换算误差；
/// 同时不依赖 set_decorations / set_size 的异步执行顺序，恢复结果确定。
#[cfg(desktop)]
fn restore_normal_geometry(window: &tauri::WebviewWindow) {
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        // 拿不到显示器信息时退回旧逻辑：逻辑尺寸 + center()
        let _ = window.set_size(tauri::LogicalSize::new(1500, 800));
        let _ = window.center();
        return;
    };

    let wa = monitor.work_area();
    let desired_w = 1500u32.min(wa.size.width);
    let desired_h = 800u32.min(wa.size.height);

    let _ = window.set_size(tauri::PhysicalSize::new(desired_w, desired_h));

    // 用实际 outer 尺寸在工作区内居中：窗口若比工作区大（理论上已被 clamp 排除），
    // 用 center() 会给出负偏移导致跑出屏幕，这里显式算位置更稳。
    if let Ok(outer) = window.outer_size() {
        let x = wa.position.x + (wa.size.width.saturating_sub(outer.width) / 2) as i32;
        let y = wa.position.y + (wa.size.height.saturating_sub(outer.height) / 2) as i32;
        let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

#[tauri::command]
// scale/app_handle 只在桌面分支（cfg(desktop) 内调整窗口）使用，
// 安卓/iOS 编译时视为未使用——用 cfg_attr 消除该平台上的警告
#[cfg_attr(not(desktop), allow(unused_variables))]
pub async fn set_pet_mode(
    enable: bool,
    scale: Option<f64>,
    app_handle: AppHandle,
    state: tauri::State<'_, HitTestState>,
) -> Result<(), String> {
    if let Ok(mut locked_enabled) = state.enabled.lock() {
        *locked_enabled = enable;
    }

    #[cfg(desktop)]
    if let Some(window) = app_handle.get_webview_window("main") {
        // 两个分支都要先退全屏：进入时防的是 issue #618 的复现路径
        // （F11 全屏 → 进桌宠 → 退出，边框消失）；退出时防的是状态脱钩，
        // 例如路由异常导致桌宠状态与全屏状态不一致时，恢复装饰同样会被吞。
        leave_fullscreen(&window).await;

        if enable {
            let scale_val = scale.unwrap_or(1.0);

            // 窗口尺寸基于桌宠组件尺寸计算：BASE_AVATAR_SIZE = 240, CHAT_BASE_H = 45, DIALOG_MAX_BASE = 200
            // GameRoleAvatar 头像框: Math.round(210 * scale)，使用标准桌宠尺寸:
            // Width: 240 * scale, Height: (240 + 200 + 45) * scale = 485 * scale
            let width = (240.0 * scale_val) as u32;
            let height = ((240.0 + 200.0 + 45.0) * scale_val) as u32;

            let _ = window.set_skip_taskbar(true);
            let _ = window.set_always_on_top(true);
            let _ = window.set_resizable(false);
            let _ = window.set_decorations(false);
            let _ = window.set_maximizable(false);
            let _ = window.set_size(LogicalSize::new(width, height));
        } else {
            // 兜底退全屏 / 取消最大化：leave_fullscreen 依赖 tao 内部全屏标志
            // （set_fullscreen(false) 一调用标志即同步清除，OS 侧可能尚未真正退出，
            //  Windows 上等待循环基本是空转），这里再补一刀让残留状态别吞掉后续 set_size。
            let _ = window.set_fullscreen(false);
            let _ = window.unmaximize();

            // Restore normal window
            let _ = window.set_maximizable(true);
            let _ = window.set_skip_taskbar(false);
            let _ = window.set_always_on_top(false);
            let _ = window.set_resizable(true);
            let _ = window.set_decorations(true);

            // 恢复 1500×800 并居中，但任何情况下不超出当前显示器工作区。
            // 之前直接 set_size(LogicalSize 1500,800)：高 DPI / 小屏下逻辑尺寸
            // 可能比工作区还大，或 set_decorations 的异步竞态让窗口停在桌宠态的
            // 不确定几何，结果窗口撑出屏幕、盖住任务栏，看起来像全屏。
            restore_normal_geometry(&window);

            // Always restore cursor ignore to false
            let _ = window.set_ignore_cursor_events(false);
        }
    }
    Ok(())
}
