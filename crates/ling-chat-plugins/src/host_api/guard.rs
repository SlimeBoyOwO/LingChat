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

thread_local! {
    /// 当前线程正在执行的插件是否声明了 `director_task` 能力。它会花掉一轮
    /// 后台导演生成，必须由 manifest 单独声明。
    static DIRECTOR_TASK_ALLOW: RefCell<bool> = const { RefCell::new(false) };
}

pub(crate) struct DirectorTaskAllowGuard {
    prev: bool,
}

impl Drop for DirectorTaskAllowGuard {
    fn drop(&mut self) {
        DIRECTOR_TASK_ALLOW.with(|cell| *cell.borrow_mut() = self.prev);
    }
}

pub(crate) fn set_director_task_allow(allow: bool) -> DirectorTaskAllowGuard {
    DIRECTOR_TASK_ALLOW.with(|cell| DirectorTaskAllowGuard {
        prev: cell.replace(allow),
    })
}

pub(super) fn current_director_task_allow() -> bool {
    DIRECTOR_TASK_ALLOW.with(|cell| *cell.borrow())
}

thread_local! {
    /// 当前线程正在执行的插件是否声明了 `script_run` 能力。它会在本机执行插件
    /// 携带的 Node 脚本，必须由 manifest 单独声明。
    static SCRIPT_RUN_ALLOW: RefCell<bool> = const { RefCell::new(false) };
}

pub(crate) struct ScriptRunAllowGuard {
    prev: bool,
}

impl Drop for ScriptRunAllowGuard {
    fn drop(&mut self) {
        SCRIPT_RUN_ALLOW.with(|cell| *cell.borrow_mut() = self.prev);
    }
}

pub(crate) fn set_script_run_allow(allow: bool) -> ScriptRunAllowGuard {
    SCRIPT_RUN_ALLOW.with(|cell| ScriptRunAllowGuard {
        prev: cell.replace(allow),
    })
}

pub(super) fn current_script_run_allow() -> bool {
    SCRIPT_RUN_ALLOW.with(|cell| *cell.borrow())
}

thread_local! {
    /// 当前线程正在执行的插件是否声明了 `plugin_view` 能力：允许把回环页面
    /// 放进应用内的沙箱窗口。
    static PLUGIN_VIEW_ALLOW: RefCell<bool> = const { RefCell::new(false) };
}

pub(crate) struct PluginViewAllowGuard {
    prev: bool,
}

impl Drop for PluginViewAllowGuard {
    fn drop(&mut self) {
        PLUGIN_VIEW_ALLOW.with(|cell| *cell.borrow_mut() = self.prev);
    }
}

pub(crate) fn set_plugin_view_allow(allow: bool) -> PluginViewAllowGuard {
    PLUGIN_VIEW_ALLOW.with(|cell| PluginViewAllowGuard {
        prev: cell.replace(allow),
    })
}

pub(super) fn current_plugin_view_allow() -> bool {
    PLUGIN_VIEW_ALLOW.with(|cell| *cell.borrow())
}
