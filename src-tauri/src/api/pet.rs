use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
#[cfg(desktop)]
use tauri::Emitter;
#[cfg(desktop)]
use tauri::LogicalSize;
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

/// 当前显示器工作区，已按与鼠标位置**完全相同**的公式换算成窗口相对逻辑像素。
///
/// 桌宠前端用它算视线衰减的参考距离（锚点沿鼠标方向到屏幕边缘有多远），所以
/// 必须与 x/y 同坐标系、同 scale_factor。前端自己读 window.screenX/availLeft 不行：
/// 混合 DPI 多显示器下 Chromium 会混用设备像素与 CSS 像素，而这是两个数的比值，
/// 分子分母必须同源误差才会相消。
#[cfg_attr(not(desktop), allow(dead_code))]
#[derive(Clone, Debug, Serialize)]
pub struct ScreenBox {
    pub left: f64,
    pub top: f64,
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
    /// 取不到显示器信息时为 None，前端据此退回径向参考距离
    pub screen: Option<ScreenBox>,
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

/// 桌宠点击穿透轮询：全局轮询鼠标位置，只有落在前端上报的 solid 区域内才接收鼠标事件，
/// 其余透明区域把点击让给底下的窗口。
///
/// 原本用 Win32 的 GetCursorPos，因此整段是 cfg(windows) 独占，macOS 上桌宠窗口
/// 会整块挡住底下窗口的点击。cursor_position() 与 set_ignore_cursor_events() 都是
/// Tauri 的跨平台 API，改用前者后三个桌面平台可以共用同一个循环。
/// （Linux 未实测：X11 / Wayland 下最差情况是 API 返回 Err，本轮直接跳过。）
///
/// 把气泡窗重新提到最前。
///
/// `always_on_top` 只是把窗口标记为 topmost，**同一 topmost 组内的前后顺序**会随焦点
/// 变化丢失：用户点了别的窗口后，气泡窗可能落到后面。而 tao 的 `set_always_on_top`
/// 只在标志**变化**时才调 `SetWindowPos`，重复设 true 是无操作 —— 所以必须自己调。
///
/// `SWP_NOACTIVATE` 保证重申置顶不会抢走用户当前窗口的焦点。
#[cfg(target_os = "windows")]
fn raise_bubble(app: &AppHandle) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        HWND_TOPMOST, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    };

    let Some(bubble) = app.get_webview_window("pet_bubble") else {
        return;
    };
    let Ok(hwnd) = bubble.hwnd() else {
        return;
    };
    // tauri 依赖的 windows crate 与本 crate 版本不同，HWND 类型不通用，
    // 只能走裸句柄转换（与 api/save.rs、cast/capture.rs 的做法一致）。
    let raw = HWND(hwnd.0 as *mut core::ffi::c_void);
    unsafe {
        let _ = SetWindowPos(
            raw,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_ASYNCWINDOWPOS | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

#[cfg(all(desktop, not(target_os = "windows")))]
fn raise_bubble(_app: &AppHandle) {}

/// 同时承担 pet:cursor 鼠标广播：向桌宠前端广播全局鼠标位置驱动 Live2D 视线。
#[cfg(desktop)]
pub fn spawn_hit_test_poll(window: tauri::WebviewWindow) {
    let hit_test_state = window.state::<HitTestState>();
    let rects_arc = hit_test_state.solid_rects.clone();
    let enabled_arc = hit_test_state.enabled.clone();
    // 供轮询里重申气泡窗置顶用（需 owned，故在进入 spawn 前取出）
    let app = window.app_handle().clone();

    tauri::async_runtime::spawn(async move {
        let mut was_ignored = false;
        // 上一次向前端广播的鼠标位置：挂机时鼠标不动，若仍 20Hz 无条件
        // emit，webview 渲染进程会被 IPC 持续唤醒而无法进入空闲。
        // 只有位移超过 1 逻辑像素（过滤亚像素抖动）才真正广播。
        let mut last_emitted: Option<(f64, f64)> = None;
        // 上一次广播的工作区矩形。原生拖拽窗口时窗口跟着光标走，窗口相对坐标几乎
        // 不变，只看鼠标位移会漏掉「参考系变了」，所以它也要参与判重。
        let mut last_screen: Option<(f64, f64, f64, f64)> = None;
        // 周期性重申气泡窗置顶：20Hz 轮询里每 ~2s 一次
        let mut ticks: u32 = 0;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            let enabled = if let Ok(locked) = enabled_arc.lock() {
                *locked
            } else {
                false
            };

            if !enabled {
                if was_ignored {
                    let _ = window.set_ignore_cursor_events(false);
                    was_ignored = false;
                }
                continue;
            }

            // 每 ~2s 重申一次气泡窗置顶：别的窗口抢焦点后它会掉到后面去
            ticks = ticks.wrapping_add(1);
            if ticks % 40 == 0 {
                raise_bubble(&app);
            }

            // 桌面全局坐标（物理像素），与 outer_position() 同一坐标系
            let Ok(cursor) = window.cursor_position() else {
                continue;
            };

            if let Ok(window_pos) = window.outer_position() {
                if let Ok(scale_factor) = window.scale_factor() {
                    let mouse_x = cursor.x - f64::from(window_pos.x);
                    let mouse_y = cursor.y - f64::from(window_pos.y);

                    let logical_x = mouse_x / scale_factor;
                    let logical_y = mouse_y / scale_factor;

                    // 当前显示器工作区，换算方式与鼠标位置逐字一致，
                    // 保证两者是同一帧的原子快照。
                    let screen = window.current_monitor().ok().flatten().map(|monitor| {
                        let area = monitor.work_area();
                        ScreenBox {
                            left: (f64::from(area.position.x) - f64::from(window_pos.x))
                                / scale_factor,
                            top: (f64::from(area.position.y) - f64::from(window_pos.y))
                                / scale_factor,
                            width: f64::from(area.size.width) / scale_factor,
                            height: f64::from(area.size.height) / scale_factor,
                        }
                    });

                    // 向桌宠前端广播全局鼠标位置：桌宠窗口非全屏，DOM
                    // pointermove 在鼠标移出窗口后停发，Live2D 视线会冻结在
                    // 最后一次窗口内位置。这里把窗口内逻辑坐标（即 webview
                    // 视口坐标）发给前端驱动视线，与 DOM clientX/Y 同坐标系。
                    // 视线弹簧在前端 ticker 内持续插值，广播间隔变大不影响
                    // 追踪平滑度，因此只在位移 ≥1px 时发送。
                    let moved = match last_emitted {
                        Some((lx, ly)) => {
                            (logical_x - lx).abs() >= 1.0 || (logical_y - ly).abs() >= 1.0
                        },
                        None => true,
                    };
                    let screen_key = screen
                        .as_ref()
                        .map(|area| (area.left, area.top, area.width, area.height));
                    if moved || screen_key != last_screen {
                        let _ = window.emit(
                            "pet:cursor",
                            CursorPosition {
                                x: logical_x,
                                y: logical_y,
                                screen,
                            },
                        );
                        last_emitted = Some((logical_x, logical_y));
                        last_screen = screen_key;
                    }

                    let mut is_over_solid = false;
                    if let Ok(rects) = rects_arc.lock() {
                        for r in rects.iter() {
                            if logical_x >= r.x
                                && logical_y >= r.y
                                && logical_x <= (r.x + r.width)
                                && logical_y <= (r.y + r.height)
                            {
                                is_over_solid = true;
                                break;
                            }
                        }
                    }

                    if is_over_solid {
                        if was_ignored {
                            let _ = window.set_ignore_cursor_events(false);
                            was_ignored = false;
                        }
                    } else {
                        if !was_ignored {
                            let _ = window.set_ignore_cursor_events(true);
                            was_ignored = true;
                        }
                    }
                }
            }
        }
    });
}

// ═══════════════════════════ 气泡窗（双窗口）═══════════════════════════
//
// 宠物窗是锚点：尺寸固定、位置只由用户拖动决定。气泡窗是独立窗口，位置**相对宠物窗
// 恒定**（底边落在宠物头顶上方 BUBBLE_GAP 处），靠监听宠物窗的移动事件跟随 ——
// 不用原生父子窗口是因为那种从属关系在三个桌面平台的 z-order 表现不一致，
// 而"按绝对坐标重算一次"到处都一样。
//
// 气泡窗整窗点击穿透：它叠在宠物窗上方，不穿透就会抢走头像的拖拽与点击。

/// 两窗共用的宽度基准：视觉上必须一致，否则气泡窗与宠物窗明显错位。
const WINDOW_W: f64 = 264.0;
/// 宠物窗高度 = 头像带 210 + 输入带 70
const PET_WINDOW_H: f64 = 280.0;
/// 气泡窗高度 = 气泡带 278 + 长尾余量 10 + 间隙 8
const BUBBLE_H: f64 = 296.0;
const BUBBLE_GAP: f64 = 8.0;

/// 按宠物窗当前位置重算气泡窗位置：气泡窗底边落在宠物窗顶边上方 `BUBBLE_GAP` 处。
///
/// 用气泡窗**实测**逻辑尺寸而不是常量，这样换缩放倍率时偏移自动跟着变。
/// 两窗共用同一 scale_factor，逻辑坐标可直接相加。
#[cfg(desktop)]
fn place_bubble(app: &AppHandle, pet: &tauri::WebviewWindow) {
    let Some(bubble) = app.get_webview_window("pet_bubble") else {
        return;
    };
    let (Ok(pos), Ok(scale)) = (pet.outer_position(), pet.scale_factor()) else {
        return;
    };
    let bubble_h = bubble
        .outer_size()
        .map(|size| f64::from(size.height) / scale)
        .unwrap_or(BUBBLE_H);
    let _ = bubble.set_position(tauri::LogicalPosition::new(
        f64::from(pos.x) / scale,
        f64::from(pos.y) / scale - bubble_h - BUBBLE_GAP,
    ));
}

#[cfg(desktop)]
fn close_bubble_window(app: &AppHandle) {
    if let Some(bubble) = app.get_webview_window("pet_bubble") {
        let _ = bubble.close();
    }
}

/// 确保气泡窗存在且尺寸正确。
///
/// **已存在时只改尺寸、不重建** —— 重建会销毁 webview，气泡里的台词与打字机状态
/// 一起丢失，改缩放滑杆时会看到气泡闪一下重新出现。
#[cfg(desktop)]
fn ensure_bubble_window(app: &AppHandle, scale: f64) -> tauri::Result<()> {
    use tauri::{LogicalSize, WebviewUrl, WebviewWindowBuilder};

    if let Some(bubble) = app.get_webview_window("pet_bubble") {
        let _ = bubble.set_size(LogicalSize::new(WINDOW_W * scale, BUBBLE_H * scale));
        return Ok(());
    }

    let bubble = WebviewWindowBuilder::new(
        app,
        "pet_bubble",
        WebviewUrl::App("index.html?window=bubble".into()),
    )
    .title("LingChat Bubble")
    .inner_size(WINDOW_W * scale, BUBBLE_H * scale)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(false)
    .visible(false)
    .build()?;

    // 气泡窗只负责画，不接收任何鼠标事件
    let _ = bubble.set_ignore_cursor_events(true);
    bubble.show()?;

    // 跟随：宠物窗每移动一次，气泡窗按固定偏移重算一次（拖动期间跟随本身的频率）。
    // 宠物窗重新获得焦点时也重申一次置顶 —— 用户从别的程序切回桌宠时，
    // topmost 组内的前后顺序会丢失，靠 2s 轮询兜底会有一段被遮挡的空窗期。
    if let Some(pet) = app.get_webview_window("main") {
        place_bubble(app, &pet);
        let handle = app.clone();
        pet.on_window_event(move |event| {
            if let tauri::WindowEvent::Focused(true) = event {
                raise_bubble(&handle);
            }
            if let tauri::WindowEvent::Moved(_) = event {
                if let Some(pet) = handle.get_webview_window("main") {
                    place_bubble(&handle, &pet);
                }
            }
        });
    }

    tracing::info!("气泡窗已创建");
    Ok(())
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

/// macOS：把主窗口标题栏样式重新断言为 Overlay（titlebar 透明 + FullSizeContentView，
/// 窗口圆角、红绿灯悬浮）。
///
/// 桌宠模式进出时 `set_decorations` 会异步重建 style mask（走主线程 GCD 队列），
/// 丢掉 Overlay 依赖的 FullSizeContentView，窗口会变回「有标题栏 + 直角角」、
/// 红绿灯被挤出画面；而 `set_title_bar_style` 是同步读当前 mask 再叠加，
/// mask 未落地时调用会读到旧态。恢复流程要在 mask 落地后、几何恢复后各断言一次。
#[cfg(target_os = "macos")]
fn reassert_overlay_title_bar(window: &tauri::WebviewWindow) {
    let _ = window.set_title_bar_style(tauri::TitleBarStyle::Overlay);
}

/// macOS：等待 `set_decorations(true)` 异步重建的 style mask 落地，再断言 Overlay。
///
/// tao 的 mask 重建没有完成回调，只能固定退让 80ms——这个时延依赖 tao 的异步
/// 实现细节，若上游改为同步应用 mask 可移除等待。仅在退出桌宠紧跟
/// `set_decorations(true)` 之后调用一次；几何恢复（set_size/set_position）后
/// 的最终断言直接用 `reassert_overlay_title_bar`，不要重复等待。
#[cfg(target_os = "macos")]
async fn wait_decoration_mask_then_reassert_overlay_title_bar(window: &tauri::WebviewWindow) {
    tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    reassert_overlay_title_bar(window);
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

            // 宠物窗 = 头像带 + 输入带，**不含气泡**：气泡独立成窗，宠物窗顶边因此
            // 就是头像顶边，可以贴到屏幕最顶。改这里必须同步 components/pet/constants.ts。
            let width = (WINDOW_W * scale_val) as u32;
            let height = (PET_WINDOW_H * scale_val) as u32;

            let _ = window.set_skip_taskbar(true);
            let _ = window.set_always_on_top(true);
            let _ = window.set_resizable(false);
            let _ = window.set_decorations(false);
            let _ = window.set_maximizable(false);
            let _ = window.set_size(LogicalSize::new(width, height));

            if let Err(e) = ensure_bubble_window(&app_handle, scale_val) {
                tracing::warn!("气泡窗创建失败，桌宠降级为无气泡: {e}");
            }
            // 改尺寸走的是"只 resize 不重建"，位置需要跟着新尺寸重算一次
            if let Some(pet) = app_handle.get_webview_window("main") {
                place_bubble(&app_handle, &pet);
            }
        } else {
            close_bubble_window(&app_handle);

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
            // macOS：set_decorations(true) 异步重建 style mask 会丢掉 Overlay 依赖的
            // FullSizeContentView（窗口变回有标题栏 + 直角角），等 mask 落地后恢复，
            // 场景与竞态细节见 wait_decoration_mask_then_reassert_overlay_title_bar。
            #[cfg(target_os = "macos")]
            wait_decoration_mask_then_reassert_overlay_title_bar(&window).await;

            // 恢复 1500×800 并居中，但任何情况下不超出当前显示器工作区。
            // 之前直接 set_size(LogicalSize 1500,800)：高 DPI / 小屏下逻辑尺寸
            // 可能比工作区还大，或 set_decorations 的异步竞态让窗口停在桌宠态的
            // 不确定几何，结果窗口撑出屏幕、盖住任务栏，看起来像全屏。
            restore_normal_geometry(&window);

            // macOS：restore_normal_geometry 会 set_size/set_position，期间可能再次
            // 覆盖掩码，这里再断言一次 Overlay，确保最终仍保留 FullSizeContentView 圆角。
            #[cfg(target_os = "macos")]
            reassert_overlay_title_bar(&window);
            // Always restore cursor ignore to false
            let _ = window.set_ignore_cursor_events(false);
        }
    }
    Ok(())
}
