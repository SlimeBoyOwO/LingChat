//! 插件视图窗口：把插件自己的回环页面放进一个沙箱窗口。
//!
//! 页面由插件的 `script_serve` 提供，宿主不注入 LingChat 前端与 Tauri IPC；
//! 只允许回环地址，导航也限制在回环 http 内。窗口关闭时回收对应的常驻脚本。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use rustpython_vm::{PyObjectRef, PyResult, VirtualMachine, function::KwArgs};
use serde_json::json;
use tauri::{Manager, Url, WebviewUrl, WebviewWindowBuilder};

use super::guard::{current_plugin, current_plugin_view_allow};
use super::script;
use super::{err_json, kwargs_map, no_host, py_to_value, runtime, value_to_pyobject};

const VIEW_WINDOW_LABEL: &str = "plugin_view";
const DEFAULT_WIDTH: f64 = 1000.0;
const DEFAULT_HEIGHT: f64 = 720.0;
const MIN_WIDTH: f64 = 480.0;
const MIN_HEIGHT: f64 = 360.0;
const MAX_TITLE_CHARS: usize = 80;

#[derive(Clone)]
struct ViewState {
    plugin_id: String,
    run_id: Option<String>,
    generation: u64,
}

static VIEW: OnceLock<Mutex<Option<ViewState>>> = OnceLock::new();
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn view_slot() -> &'static Mutex<Option<ViewState>> {
    VIEW.get_or_init(|| Mutex::new(None))
}

fn take_view() -> Option<ViewState> {
    view_slot().lock().ok()?.take()
}

fn set_view(state: ViewState) {
    if let Ok(mut slot) = view_slot().lock() {
        *slot = Some(state);
    }
}

fn is_loopback_http(url: &Url) -> bool {
    url.scheme() == "http"
        && matches!(
            url.host_str(),
            Some("127.0.0.1") | Some("localhost") | Some("::1")
        )
}

fn number_arg(
    kwargs: &std::collections::HashMap<String, PyObjectRef>,
    vm: &VirtualMachine,
    key: &str,
) -> Option<f64> {
    kwargs
        .get(key)
        .map(|value| py_to_value(vm, value))
        .and_then(|value| value.as_f64())
}

fn stop_view_run(state: ViewState) {
    if let Some(run_id) = state.run_id {
        let _ = runtime().block_on(script::stop_run(&run_id, None));
    }
}

/// 窗口被用户关掉后回收服务。只处理创建这个窗口的那一代，避免误停新页面。
async fn on_destroyed(_app: tauri::AppHandle, generation: u64) {
    let current = view_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|state| state.generation));
    if current != Some(generation) {
        return;
    }
    if let Some(state) = take_view()
        && let Some(run_id) = state.run_id
    {
        let _ = script::stop_run(&run_id, None).await;
    }
}

pub(super) fn open_plugin_view(
    url: String,
    kwargs: KwArgs<PyObjectRef>,
    vm: &VirtualMachine,
) -> PyResult<PyObjectRef> {
    if !current_plugin_view_allow() {
        return Ok(err_json(
            vm,
            "本插件未声明视图窗口能力（需在 manifest.toml 顶层写 plugin_view = true）",
        ));
    }
    let Some(app) = crate::app_handle() else {
        return Ok(no_host(vm));
    };
    let Some(plugin_id) = current_plugin() else {
        return Ok(err_json(vm, "宿主不知道当前是哪个插件"));
    };

    let parsed = match Url::parse(url.trim()) {
        Ok(url) => url,
        Err(error) => return Ok(err_json(vm, &format!("URL 不合法：{error}"))),
    };
    if !is_loopback_http(&parsed) {
        return Ok(err_json(
            vm,
            "只允许打开 http://127.0.0.1 / localhost 的回环页面",
        ));
    }

    let kwargs = kwargs_map(kwargs);
    let title = kwargs
        .get("title")
        .and_then(|value| py_to_value(vm, value).as_str().map(str::to_string))
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| "插件页面".to_string());
    let title: String = title.chars().take(MAX_TITLE_CHARS).collect();
    let width = number_arg(&kwargs, vm, "width")
        .unwrap_or(DEFAULT_WIDTH)
        .clamp(MIN_WIDTH, 1600.0);
    let height = number_arg(&kwargs, vm, "height")
        .unwrap_or(DEFAULT_HEIGHT)
        .clamp(MIN_HEIGHT, 1200.0);
    let run_id = kwargs
        .get("run_id")
        .and_then(|value| {
            py_to_value(vm, value)
                .as_str()
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|run_id| !run_id.is_empty());

    if let Some(run_id) = run_id.as_deref() {
        let owner = runtime().block_on(script::run_owner(run_id));
        if owner.as_deref() != Some(plugin_id.as_str()) {
            return Ok(err_json(vm, "run_id 不存在，或不属于当前插件"));
        }
    }

    // 新页面顶掉旧页面时，先回收旧页面绑定的常驻服务。
    let previous = take_view();
    if let Some(previous) = previous.clone()
        && previous.run_id != run_id
    {
        stop_view_run(previous);
    }

    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let built = if let Some(window) = app.get_webview_window(VIEW_WINDOW_LABEL) {
        let _ = window.set_title(&title);
        window.navigate(parsed).map(|_| window)
    } else {
        WebviewWindowBuilder::new(&app, VIEW_WINDOW_LABEL, WebviewUrl::External(parsed))
            .title(&title)
            .inner_size(width, height)
            .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
            .on_navigation(|url| is_loopback_http(url))
            .build()
    };

    match built {
        Ok(window) => {
            let app_for_event = app.clone();
            window.on_window_event(move |event| {
                if matches!(event, tauri::WindowEvent::Destroyed) {
                    let app = app_for_event.clone();
                    tauri::async_runtime::spawn(async move { on_destroyed(app, generation).await });
                }
            });
            set_view(ViewState {
                plugin_id,
                run_id,
                generation,
            });
            Ok(value_to_pyobject(vm, &json!({ "ok": true })))
        },
        Err(error) => {
            // 窗口没建起来：旧页面已关，旧的常驻服务不要留着。
            if let Some(previous) = previous
                && let Some(run_id) = previous.run_id
            {
                let _ = runtime().block_on(script::stop_run(&run_id, None));
            }
            Ok(err_json(vm, &format!("创建插件视图窗口失败：{error}")))
        },
    }
}

/// 插件禁用/删除时关闭它打开的视图窗口。
pub(crate) async fn close_for_plugin(plugin_id: &str) {
    let matches = view_slot()
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|state| state.plugin_id == plugin_id))
        .unwrap_or(false);
    if !matches {
        return;
    }
    let Some(state) = take_view() else {
        return;
    };
    if let Some(app) = crate::app_handle()
        && let Some(window) = app.get_webview_window(VIEW_WINDOW_LABEL)
    {
        let _ = window.destroy();
    }
    if let Some(run_id) = state.run_id {
        let _ = script::stop_run(&run_id, None).await;
    }
}
