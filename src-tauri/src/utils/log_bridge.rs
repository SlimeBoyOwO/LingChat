use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::OnceLock;

use chrono::Local;
use tauri::AppHandle;
use tauri::Emitter;
use tracing::field::{Field, Visit};
use tracing_subscriber::Layer;

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// Maximum number of log entries kept in the ring buffer.
const MAX_BUFFER: usize = 2000;

/// Ring buffer of log entries since program start.
/// Always populated — the frontend fetches this on mount to catch startup logs
/// that were emitted before the webview was ready.
static LOG_BUFFER: once_cell::sync::Lazy<Mutex<VecDeque<LogEntry>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(VecDeque::new()));

pub fn set_app_handle(handle: AppHandle) {
    let _ = APP_HANDLE.set(handle);
}

/// Returns all buffered log entries (up to 2000) from program start.
#[tauri::command]
pub fn get_log_history() -> Vec<LogEntry> {
    let buffer = LOG_BUFFER.lock().unwrap();
    buffer.iter().cloned().collect()
}

/// 打开独立日志窗口；已存在时聚焦（仅桌面端，移动端为 no-op）。
#[tauri::command]
pub async fn open_log_window(app: AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

        if let Some(win) = app.get_webview_window("log") {
            let _ = win.unminimize();
            win.set_focus().map_err(|e| format!("聚焦日志窗口失败: {}", e))?;
            return Ok(());
        }

        let win = WebviewWindowBuilder::new(
            &app,
            "log",
            WebviewUrl::App("index.html?window=log".into()),
        )
        .title("日志")
        .inner_size(960.0, 640.0)
        .min_inner_size(480.0, 320.0)
        .build()
        .map_err(|e| format!("创建日志窗口失败: {}", e))?;

        // 广播窗口状态，供设置页切换“已弹出”占位提示
        let app_for_event = app.clone();
        win.on_window_event(move |event| {
            if let tauri::WindowEvent::Destroyed = event {
                let _ = app_for_event.emit("log-window:state", false);
            }
        });
        let _ = app.emit("log-window:state", true);

        tracing::info!("[LogWindow] 独立日志窗口已创建");
    }
    Ok(())
}

/// 查询独立日志窗口是否已打开（移动端恒为 false）。
#[tauri::command]
pub fn is_log_window_open(app: AppHandle) -> bool {
    #[cfg(desktop)]
    {
        use tauri::Manager;
        app.get_webview_window("log").is_some()
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
        false
    }
}

#[derive(Clone, serde::Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub target: String,
    pub message: String,
}

struct LogVisitor {
    message: String,
    fields: Vec<(String, String)>,
}

impl Default for LogVisitor {
    fn default() -> Self {
        Self {
            message: String::new(),
            fields: Vec::new(),
        }
    }
}

impl Visit for LogVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let s = format!("{:?}", value);
        if field.name() == "message" {
            self.message = s;
        } else {
            self.fields.push((field.name().to_string(), s));
        }
    }
}

/// A tracing Layer that buffers log entries and emits them to the Tauri frontend.
pub struct LogBridgeLayer;

impl<S> Layer<S> for LogBridgeLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();

        let mut visitor = LogVisitor::default();
        event.record(&mut visitor);

        let display_message = if !visitor.message.is_empty() {
            if visitor.fields.is_empty() {
                visitor.message
            } else {
                let kv = visitor
                    .fields
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{} {}", visitor.message, kv)
            }
        } else if !visitor.fields.is_empty() {
            visitor
                .fields
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            String::new()
        };

        let entry = LogEntry {
            timestamp: Local::now().format("%H:%M:%S%.3f").to_string(),
            level: metadata.level().to_string(),
            target: metadata.target().to_string(),
            message: display_message,
        };

        // Always buffer
        {
            let mut buffer = LOG_BUFFER.lock().unwrap();
            buffer.push_back(entry.clone());
            if buffer.len() > MAX_BUFFER {
                buffer.pop_front();
            }
        }

        // Emit to frontend if the handle is available
        if let Some(handle) = APP_HANDLE.get() {
            let _ = handle.emit("log:entry", entry);
        }
    }
}
