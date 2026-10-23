//! 插件脚本执行期间的线程本地能力开关。
//!
//! 每次执行由 `python_backend::run_entry` 设置、guard 在 Drop 时还原；嵌套执行
//! （脚本里 `call_tool` 触发另一个插件）也不会串味。

// 交叉 target 下 clippy 会对已用 const 初始化的 thread_local 误报（issue #13422）。
#![allow(clippy::missing_const_for_thread_local)]

use std::cell::RefCell;

thread_local! {
    /// 当前线程正在执行的插件是否声明了 `trigger_proactive` 能力。
    static PROACTIVE_ALLOW: RefCell<bool> = const { RefCell::new(false) };
}

pub(crate) struct ProactiveAllowGuard {
    prev: bool,
}

impl Drop for ProactiveAllowGuard {
    fn drop(&mut self) {
        PROACTIVE_ALLOW.with(|cell| *cell.borrow_mut() = self.prev);
    }
}

pub(crate) fn set_proactive_allow(allow: bool) -> ProactiveAllowGuard {
    PROACTIVE_ALLOW.with(|cell| ProactiveAllowGuard {
        prev: cell.replace(allow),
    })
}

pub(super) fn current_proactive_allow() -> bool {
    PROACTIVE_ALLOW.with(|cell| *cell.borrow())
}

thread_local! {
    /// 当前线程正在执行的插件所声明的可读前缀。
    ///
    /// 脚本在单个线程内同步跑完，`call_tool` 会切到插件 runtime 的线程再切回来，
    /// 所以「当前插件」按线程区分是成立的。
    static READ_ALLOW: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct ReadAllowGuard {
    prev: Vec<String>,
}

impl Drop for ReadAllowGuard {
    fn drop(&mut self) {
        READ_ALLOW.with(|cell| *cell.borrow_mut() = std::mem::take(&mut self.prev));
    }
}

pub(crate) fn set_read_allow(allow: &[String]) -> ReadAllowGuard {
    let prev = READ_ALLOW.with(|cell| std::mem::replace(&mut *cell.borrow_mut(), allow.to_vec()));
    ReadAllowGuard { prev }
}

pub(super) fn current_read_allow() -> Vec<String> {
    READ_ALLOW.with(|cell| cell.borrow().clone())
}

thread_local! {
    /// 当前线程正在执行的插件 id。`ws_send` / `ws_open` / `ws_close` 按插件定位连接。
    static CURRENT_PLUGIN: RefCell<Option<String>> = const { RefCell::new(None) };
}

pub(crate) struct CurrentPluginGuard {
    prev: Option<String>,
}

impl Drop for CurrentPluginGuard {
    fn drop(&mut self) {
        CURRENT_PLUGIN.with(|cell| *cell.borrow_mut() = std::mem::take(&mut self.prev));
    }
}

pub(crate) fn set_current_plugin(id: &str) -> CurrentPluginGuard {
    let prev = CURRENT_PLUGIN.with(|cell| cell.borrow_mut().replace(id.to_string()));
    CurrentPluginGuard { prev }
}

pub(super) fn current_plugin() -> Option<String> {
    CURRENT_PLUGIN.with(|cell| cell.borrow().clone())
}

thread_local! {
    /// 当前线程正在执行的插件是否声明了 `send_user_message` 能力。
    static SEND_ALLOW: RefCell<bool> = const { RefCell::new(false) };
}

pub(crate) struct SendAllowGuard {
    prev: bool,
}

impl Drop for SendAllowGuard {
    fn drop(&mut self) {
        SEND_ALLOW.with(|cell| *cell.borrow_mut() = self.prev);
    }
}

pub(crate) fn set_send_allow(allow: bool) -> SendAllowGuard {
    SEND_ALLOW.with(|cell| SendAllowGuard {
        prev: cell.replace(allow),
    })
}

pub(super) fn current_send_allow() -> bool {
    SEND_ALLOW.with(|cell| *cell.borrow())
}

thread_local! {
    /// 当前线程正在执行的插件是否声明了 `capture_screen` 能力。抓屏是玩家隐私，
    /// 必须由 manifest 单独声明，未声明一律拒绝。
    static CAPTURE_ALLOW: RefCell<bool> = const { RefCell::new(false) };
}

pub(crate) struct CaptureAllowGuard {
    prev: bool,
}

impl Drop for CaptureAllowGuard {
    fn drop(&mut self) {
        CAPTURE_ALLOW.with(|cell| *cell.borrow_mut() = self.prev);
    }
}

pub(crate) fn set_capture_allow(allow: bool) -> CaptureAllowGuard {
    CAPTURE_ALLOW.with(|cell| CaptureAllowGuard {
        prev: cell.replace(allow),
    })
}

pub(super) fn current_capture_allow() -> bool {
    CAPTURE_ALLOW.with(|cell| *cell.borrow())
}
