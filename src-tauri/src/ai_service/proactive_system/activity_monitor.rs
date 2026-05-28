use crate::ai_service::proactive_system::types::{PerceptionResult, UserState};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_MBUTTONDOWN,
    WM_MOUSEMOVE, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
};

#[cfg(target_os = "windows")]
#[derive(Clone, Copy, Debug)]
pub struct SendHhook(pub HHOOK);

#[cfg(target_os = "windows")]
unsafe impl Send for SendHhook {}
#[cfg(target_os = "windows")]
unsafe impl Sync for SendHhook {}

#[derive(Clone, Debug)]
enum InputType {
    Key { is_game: bool },
    Click,
    Move { x: i32, y: i32 },
}

#[derive(Clone, Debug)]
struct ActivityEvent {
    timestamp: Instant,
    input_type: InputType,
}

struct MonitorInner {
    events: Vec<ActivityEvent>,
    last_mouse_pos: Option<(i32, i32)>,
}

pub struct UserActivityMonitor {
    inner: Arc<Mutex<MonitorInner>>,
    #[cfg(target_os = "windows")]
    _kbd_hook: Option<SendHhook>,
    #[cfg(target_os = "windows")]
    _ms_hook: Option<SendHhook>,
}

// Win32 hooks require a static/global callback, so we use a OnceLock to access the active monitor's inner state.
static GLOBAL_MONITOR_INNER: OnceLock<Arc<Mutex<MonitorInner>>> = OnceLock::new();

impl UserActivityMonitor {
    pub fn new() -> Self {
        let inner = Arc::new(Mutex::new(MonitorInner {
            events: Vec::new(),
            last_mouse_pos: None,
        }));

        GLOBAL_MONITOR_INNER.get_or_init(|| inner.clone());

        #[cfg(target_os = "windows")]
        {
            let mut monitor = Self {
                inner,
                _kbd_hook: None,
                _ms_hook: None,
            };
            monitor.start_monitoring();
            monitor
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self { inner }
        }
    }

    #[cfg(target_os = "windows")]
    fn start_monitoring(&mut self) {
        let kbd_hook = unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_callback), None, 0)
                .ok()
                .map(SendHhook)
        };

        let ms_hook = unsafe {
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_callback), None, 0)
                .ok()
                .map(SendHhook)
        };

        if kbd_hook.is_some() && ms_hook.is_some() {
            tracing::info!("[ActivityMonitor] Win32 hooks installed successfully!");
        } else {
            tracing::error!(
                "[ActivityMonitor] Failed to install Win32 hooks: kbd={:?}, ms={:?}",
                kbd_hook,
                ms_hook
            );
        }

        self._kbd_hook = kbd_hook;
        self._ms_hook = ms_hook;

        // Spawn a Win32 message pump thread to receive hook events
        std::thread::spawn(|| {
            unsafe {
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    // Just pump messages
                }
            }
        });
    }

    /// 清理 20 秒之前的旧事件并计算统计信息。
    pub fn get_user_status(&self) -> PerceptionResult {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let cutoff = now - Duration::from_secs(20);

        // Slide window
        inner.events.retain(|e| e.timestamp >= cutoff);

        let mut keystrokes = 0;
        let mut game_keys = 0;
        let mut clicks = 0;
        let mut mouse_distance = 0.0;

        let mut running_last_pos = inner.last_mouse_pos;
        for event in &inner.events {
            match &event.input_type {
                InputType::Key { is_game } => {
                    keystrokes += 1;
                    if *is_game {
                        game_keys += 1;
                    }
                }
                InputType::Click => {
                    clicks += 1;
                }
                InputType::Move { x, y } => {
                    if let Some((lx, ly)) = running_last_pos {
                        let dx = (x - lx) as f64;
                        let dy = (y - ly) as f64;
                        mouse_distance += (dx * dx + dy * dy).sqrt();
                    }
                    running_last_pos = Some((*x, *y));
                }
            }
        }
        inner.last_mouse_pos = running_last_pos;

        let game_ratio = if keystrokes > 0 {
            game_keys as f64 / keystrokes as f64
        } else {
            0.0
        };

        tracing::info!(
            "[ActivityMonitor] Stats last 20s: Keys={}, GameKeys={}, Clicks={}, Distance={:.1}, GameRatio={:.2}",
            keystrokes, game_keys, clicks, mouse_distance, game_ratio
        );

        // Classification Logic matching Python
        let state;
        let description;
        let interest_modifier;

        if mouse_distance < 10.0 && keystrokes == 0 && clicks == 0 {
            state = UserState::IDLE;
            description = "挂机发呆".to_string();
            interest_modifier = 5;
        } else if keystrokes < 5 && (mouse_distance > 2000.0 || clicks > 2) {
            state = UserState::BROWSING;
            description = "在网上冲浪".to_string();
            interest_modifier = -2;
        } else if keystrokes >= 5 {
            let is_gaming =
                (game_ratio >= 0.6 && keystrokes > 20) || (clicks > 30 && game_ratio >= 0.4);
            if is_gaming {
                state = UserState::GAME;
                description = "在打游戏".to_string();
                interest_modifier = 5;
            } else {
                state = UserState::WORK;
                description = "在认真工作/学习".to_string();
                interest_modifier = -5;
            }
        } else {
            state = UserState::CASUAL;
            description = "轻度活动".to_string();
            interest_modifier = 0;
        }

        tracing::info!(
            "[ActivityMonitor] State={} desc={} modifier={}",
            state.as_str(),
            description,
            interest_modifier
        );

        PerceptionResult {
            state,
            description,
            interest_modifier,
            visual_change_detected: false, // Updated by visual monitor
            current_screen_text: String::new(),
        }
    }
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_callback(
    code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = w_param.0 as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kbd_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
            let vk = kbd_struct.vkCode;

            // Game Keys: W(0x57), A(0x41), S(0x53), D(0x44), Space(0x20), Left(0x25), Up(0x26), Right(0x27), Down(0x28)
            let is_game = matches!(
                vk,
                0x57 | 0x41 | 0x53 | 0x44 | 0x20 | 0x25 | 0x26 | 0x27 | 0x28
            );

            if let Some(inner_arc) = GLOBAL_MONITOR_INNER.get() {
                if let Ok(mut inner) = inner_arc.lock() {
                    inner.events.push(ActivityEvent {
                        timestamp: Instant::now(),
                        input_type: InputType::Key { is_game },
                    });
                }
            }
        }
    }
    CallNextHookEx(None, code, w_param, l_param)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn mouse_hook_callback(
    code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if code >= 0 {
        let msg = w_param.0 as u32;
        let ms_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
        let pt = ms_struct.pt;

        if let Some(inner_arc) = GLOBAL_MONITOR_INNER.get() {
            if let Ok(mut inner) = inner_arc.lock() {
                if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
                    inner.events.push(ActivityEvent {
                        timestamp: Instant::now(),
                        input_type: InputType::Click,
                    });
                } else if msg == WM_MOUSEMOVE {
                    inner.events.push(ActivityEvent {
                        timestamp: Instant::now(),
                        input_type: InputType::Move { x: pt.x, y: pt.y },
                    });
                }
            }
        }
    }
    CallNextHookEx(None, code, w_param, l_param)
}

#[cfg(target_os = "windows")]
impl Drop for UserActivityMonitor {
    fn drop(&mut self) {
        tracing::info!("[ActivityMonitor] Cleaning up Win32 hooks...");
        unsafe {
            if let Some(hook) = self._kbd_hook {
                let _ = UnhookWindowsHookEx(hook.0);
            }
            if let Some(hook) = self._ms_hook {
                let _ = UnhookWindowsHookEx(hook.0);
            }
        }
    }
}
