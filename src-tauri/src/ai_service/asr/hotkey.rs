//! 系统级全局快捷键（Windows RegisterHotKey）。
//!
//! 需求：快捷键在 LingChat **后台 / 最小化** 时也能触发 —— 前端 window keydown
//! 监听只在窗口聚焦时生效，无法满足。
//!
//! 实现：`RegisterHotKey(None, id, mods, vk)` 注册到系统（无窗口依赖），后台线程
//! `PeekMessageW` 非阻塞轮询 `WM_HOTKEY`。收到按下事件 → emit `asr://hotkey_down`；
//! 随后轮询 `GetAsyncKeyState` 直到键释放 → emit `asr://hotkey_up`。
//! 前端 useAsrInput 监听这两个事件驱动 start('hotkey') / stop()。
//!
//! 组合格式（与前端 `recordKeyUntilEscape` 输出一致）：`Ctrl+Shift+Space` /
//! `Alt+F1` / `a` 等。
//!
//! 依赖：`windows` crate 已有（桌宠 hit-test 在用），仅扩展 feature
//! `Win32_UI_Input_KeyboardAndMouse`（GetAsyncKeyState / VIRTUAL_KEY / RegisterHotKey）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use tokio::sync::Mutex;

use super::error::AsrError;

/// 系统级全局快捷键管理器（每实例同时注册一组组合）。
pub struct AsrHotkey {
    /// 线程运行标志；unregister 时置 false，轮询线程自然退出。
    active: Arc<AtomicBool>,
    /// 快捷键注册 id（任意正整数，与消息 wParam 对应）。
    hotkey_id: i32,
    /// 后台监听线程句柄。
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl Default for AsrHotkey {
    fn default() -> Self {
        Self::new()
    }
}

impl AsrHotkey {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            hotkey_id: 0x4153, // "AS" magic
            handle: Mutex::new(None),
        }
    }

    /// 注册全局快捷键（先注销旧的，幂等）。
    pub async fn register(&self, app: &tauri::AppHandle, combo: &str) -> Result<(), AsrError> {
        self.unregister().await?;

        #[cfg(target_os = "windows")]
        {
            let (mods, vk) = parse_combo(combo)
                .ok_or_else(|| AsrError::EngineLoadFailed(format!("无效快捷键组合: {combo}")))?;
            let active = self.active.clone();
            let hotkey_id = self.hotkey_id;
            let app_clone = app.clone();
            let handle =
                std::thread::spawn(move || run_hotkey_thread(app_clone, mods, vk, hotkey_id, active));
            *self.handle.lock().await = Some(handle);
        }

        #[cfg(not(target_os = "windows"))]
        {
            // 非 Windows 平台暂不支持系统级热键，静默成功（前端仍可走窗口内快捷键）
            let _ = (app, combo);
        }
        Ok(())
    }

    /// 注销全局快捷键（幂等）。
    pub async fn unregister(&self) -> Result<(), AsrError> {
        let mut guard = self.handle.lock().await;
        if let Some(handle) = guard.take() {
            self.active.store(false, Ordering::SeqCst);
            let _ = handle.join();
        }
        Ok(())
    }
}

/// 组合字符串 → (修饰键标志, 虚拟键码)。与前端 eventToCombo 输出对齐：
/// "Ctrl+Shift+Space" / "Alt+F1" / "a" / "0"。
#[cfg(target_os = "windows")]
fn parse_combo(combo: &str) -> Option<(u32, u16)> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT};

    let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();
    let mut mods = 0u32;
    let mut vk: Option<u16> = None;
    for (i, p) in parts.iter().enumerate() {
        let lower = p.to_lowercase();
        match lower.as_str() {
            "ctrl" | "control" => mods |= MOD_CONTROL.0,
            "shift" => mods |= MOD_SHIFT.0,
            "alt" => mods |= MOD_ALT.0,
            _ => {
                if i == parts.len() - 1 {
                    vk = Some(key_to_vk(p)?);
                } else {
                    return None;
                }
            }
        }
    }
    let vk = vk?;
    // MOD_NOREPEAT：按住不重复触发（需 Win7+）
    mods |= MOD_NOREPEAT.0;
    Some((mods, vk))
}

/// 虚拟键码映射（覆盖 recordKeyUntilEscape 可能的输出）。
#[cfg(target_os = "windows")]
fn key_to_vk(key: &str) -> Option<u16> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    let k = key.to_lowercase();
    let single = k.chars().next()?;
    match k.as_str() {
        "space" => Some(VK_SPACE.0),
        "enter" | "return" => Some(VK_RETURN.0),
        "escape" | "esc" => Some(VK_ESCAPE.0),
        "tab" => Some(VK_TAB.0),
        "backspace" => Some(VK_BACK.0),
        "delete" | "del" => Some(VK_DELETE.0),
        "home" => Some(VK_HOME.0),
        "end" => Some(VK_END.0),
        "pageup" => Some(VK_PRIOR.0),
        "pagedown" => Some(VK_NEXT.0),
        "up" | "arrowup" => Some(VK_UP.0),
        "down" | "arrowdown" => Some(VK_DOWN.0),
        "left" | "arrowleft" => Some(VK_LEFT.0),
        "right" | "arrowright" => Some(VK_RIGHT.0),
        _ => {
            // F1-F24
            if let Some(num) = k.strip_prefix('f') {
                if let Ok(n) = num.parse::<u8>() {
                    if (1..=24).contains(&n) {
                        return Some(VK_F1.0 + (n - 1) as u16);
                    }
                }
            }
            // 单个可打印字符：a-z / 0-9 直接映射到对应虚拟键
            if single.is_ascii_alphanumeric() {
                return Some(single.to_ascii_uppercase() as u16);
            }
            None
        }
    }
}

/// 后台热键监听线程（仅 Windows）：RegisterHotKey + PeekMessage 非阻塞轮询。
#[cfg(target_os = "windows")]
fn run_hotkey_thread(
    app: tauri::AppHandle,
    mods: u32,
    vk: u16,
    hotkey_id: i32,
    active: Arc<AtomicBool>,
) {
    use tauri::Emitter;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY, WM_QUIT,
    };

    unsafe {
        let registered = RegisterHotKey(None, hotkey_id, HOT_KEY_MODIFIERS(mods), vk as u32);
        if let Err(e) = registered {
            tracing::warn!("[ASR] RegisterHotKey 失败: {e:?}");
            let _ = app.emit("asr://hotkey_register_failed", ());
            return;
        }

        tracing::info!("[ASR] 全局快捷键已注册: mods=0x{mods:x} vk=0x{vk:x}");

        let mut msg = MSG::default();
        'outer: while active.load(Ordering::SeqCst) {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'outer;
                }
                if msg.message == WM_HOTKEY && msg.wParam.0 as i32 == hotkey_id {
                    let _ = app.emit("asr://hotkey_down", ());
                    // 轮询检测键释放（RegisterHotKey 只有按下通知）
                    // GetAsyncKeyState 返回 SHORT：最高位（符号位）为 1 = 按下
                    loop {
                        if !active.load(Ordering::SeqCst) {
                            break;
                        }
                        let state = GetAsyncKeyState(vk as i32);
                        if state >= 0 {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    let _ = app.emit("asr://hotkey_up", ());
                }
            }
            if !active.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        let _ = UnregisterHotKey(None, hotkey_id);
        tracing::info!("[ASR] 全局快捷键已注销");
    }
}
