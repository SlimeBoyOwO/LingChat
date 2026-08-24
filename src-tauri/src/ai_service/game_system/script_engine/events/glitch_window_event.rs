//! Bounded auxiliary glitch windows for horror scripts.
//!
//! This is an explicit LingChat adaptation, not a claim that DDLC spawned real
//! command prompts. Windows always load the bundled, inert
//! `script-glitch.html`; scripts cannot supply URLs, HTML, labels, shell
//! commands, fullscreen state, or unbounded geometry/lifetime.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;

use crate::ai_service::game_system::script_engine::events::{
    parse_duration, register_event, ScriptContext, ScriptEvent,
};
use crate::ai_service::game_system::script_engine::responses::{
    event_names::SCRIPT_GLITCH_WINDOW, GlitchWindowTicketPayload,
};
use crate::ai_service::message_system::events::emit;

const MAX_WINDOWS: usize = 4;
const MAX_TITLE_CHARS: usize = 80;
const MAX_TEXT_CHARS: usize = 1200;
const MAX_LIFETIME_SECONDS: f64 = 12.0;
const MAX_INTERVAL_SECONDS: f64 = 1.0;
const MAX_PENDING_REQUESTS: usize = 64;
const WINDOW_LABEL_PREFIX: &str = "script-glitch-";

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
static WINDOW_GENERATION: AtomicU64 = AtomicU64::new(1);
static PENDING_WINDOWS: Mutex<Vec<(u64, u64, GlitchWindowEvent)>> = Mutex::new(Vec::new());
#[cfg(desktop)]
static WINDOW_CREATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct GlitchWindowPayload {
    title: String,
    text: String,
    style: String,
    index: usize,
    total: usize,
}

#[derive(Debug, Clone)]
pub struct GlitchWindowEvent {
    title: String,
    text: String,
    style: String,
    count: usize,
    lifetime: f64,
    interval: f64,
    duration: Option<f64>,
}

impl GlitchWindowEvent {
    fn from_event_data(data: &Value) -> Self {
        let finite = |value: Option<f64>, default: f64, max: f64| {
            let value = value.unwrap_or(default);
            if value.is_finite() {
                value.clamp(0.0, max)
            } else {
                default
            }
        };
        Self {
            title: data
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("LingChat Runtime")
                .trim()
                .to_string(),
            text: data
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("PROCESS STATE DESYNCHRONIZED")
                .to_string(),
            style: data
                .get("style")
                .and_then(Value::as_str)
                .unwrap_or("terminal")
                .trim()
                .to_ascii_lowercase(),
            count: data
                .get("count")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .clamp(1, MAX_WINDOWS as u64) as usize,
            lifetime: finite(
                data.get("lifetime").and_then(Value::as_f64),
                4.0,
                MAX_LIFETIME_SECONDS,
            )
            .max(0.5),
            interval: finite(
                data.get("interval").and_then(Value::as_f64),
                0.18,
                MAX_INTERVAL_SECONDS,
            ),
            duration: parse_duration(data),
        }
    }

    fn validate(&self) -> Result<()> {
        if !matches!(self.style.as_str(), "terminal" | "error") {
            return Err(anyhow!(
                "glitch_window.style 只支持 terminal / error，收到 '{}'",
                self.style
            ));
        }
        if self.title.is_empty()
            || self.title.chars().count() > MAX_TITLE_CHARS
            || self.title.chars().any(char::is_control)
        {
            return Err(anyhow!(
                "glitch_window.title 必须是 1-80 字符且不含控制字符"
            ));
        }
        if self.text.chars().count() > MAX_TEXT_CHARS
            || self
                .text
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
        {
            return Err(anyhow!(
                "glitch_window.text 最多 1200 字符且不能含危险控制字符"
            ));
        }
        Ok(())
    }
}

fn queue_pending_window(event: GlitchWindowEvent, generation: u64) -> Result<u64> {
    if WINDOW_GENERATION.load(Ordering::Acquire) != generation {
        return Err(anyhow!("glitch window script run has already been cancelled"));
    }
    let request_id = NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed);
    let mut pending = PENDING_WINDOWS
        .lock()
        .map_err(|_| anyhow!("glitch window request queue poisoned"))?;
    if pending.len() >= MAX_PENDING_REQUESTS {
        pending.remove(0);
        tracing::warn!(
            "[GlitchWindowEvent] 待显示窗口票据超过 {}，已丢弃最旧请求",
            MAX_PENDING_REQUESTS
        );
    }
    // Recheck while holding the pending queue lock. close_all advances the
    // generation before taking this same lock, so no cancelled ticket can slip in.
    if WINDOW_GENERATION.load(Ordering::Acquire) != generation {
        return Err(anyhow!("glitch window script run was cancelled during queueing"));
    }
    pending.push((request_id, generation, event));
    Ok(request_id)
}

fn take_pending_window(request_id: u64) -> Result<(GlitchWindowEvent, u64)> {
    let mut pending = PENDING_WINDOWS
        .lock()
        .map_err(|_| anyhow!("glitch window request queue poisoned"))?;
    let index = pending
        .iter()
        .position(|(id, _, _)| *id == request_id)
        .ok_or_else(|| anyhow!("glitch window request {request_id} is missing or already consumed"))?;
    let (_, generation, event) = pending.remove(index);
    Ok((event, generation))
}

fn discard_pending_window(request_id: u64) {
    if let Ok(mut pending) = PENDING_WINDOWS.lock() {
        if let Some(index) = pending.iter().position(|(id, _, _)| *id == request_id) {
            pending.remove(index);
        }
    }
}

fn clear_pending_windows() {
    if let Ok(mut pending) = PENDING_WINDOWS.lock() {
        pending.clear();
    }
}

pub(crate) fn script_allows_system_effects(status: &crate::ai_service::types::ScriptStatus) -> bool {
    status.content_warning.as_deref() == Some("horror")
        && status
            .settings
            .get("allow_system_effects")
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

#[cfg(desktop)]
async fn create_windows(event: &GlitchWindowEvent, app: &tauri::AppHandle, generation: u64) {
    use base64::Engine;
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    let run_id = NEXT_WINDOW_ID.fetch_add(1, Ordering::Relaxed);
    for index in 0..event.count {
        if WINDOW_GENERATION.load(Ordering::Acquire) != generation {
            tracing::info!("[GlitchWindowEvent] 窗口创建已被剧本收尾取消");
            break;
        }
        let payload = GlitchWindowPayload {
            title: event.title.clone(),
            text: event.text.clone(),
            style: event.style.clone(),
            index: index + 1,
            total: event.count,
        };
        let Ok(json) = serde_json::to_vec(&payload) else {
            tracing::warn!("[GlitchWindowEvent] 序列化窗口内容失败");
            continue;
        };
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json);
        let label = format!("{WINDOW_LABEL_PREFIX}{run_id}-{index}");
        let url = format!("script-glitch.html?payload={encoded}");
        let x = 80.0 + ((run_id as usize * 37 + index * 91) % 620) as f64;
        let y = 70.0 + ((run_id as usize * 53 + index * 67) % 360) as f64;

        {
            // The four-window limit is global across overlapping events, not
            // merely per YAML event. Serializing count+build closes the race
            // between consecutive preview/runtime tasks, while enumeration
            // counts only windows that were actually created and remain live.
            let _create_guard = WINDOW_CREATE_LOCK.lock().await;
            if WINDOW_GENERATION.load(Ordering::Acquire) != generation {
                break;
            }
            let active = app
                .webview_windows()
                .keys()
                .filter(|label| label.starts_with(WINDOW_LABEL_PREFIX))
                .count();
            if active >= MAX_WINDOWS {
                tracing::warn!(
                    "[GlitchWindowEvent] 全局辅助窗口上限 {} 已满，本事件剩余窗口已跳过",
                    MAX_WINDOWS
                );
                break;
            }

            match WebviewWindowBuilder::new(app, &label, WebviewUrl::App(url.into()))
                .title(&event.title)
                .inner_size(560.0, if event.style == "error" { 260.0 } else { 300.0 })
                .min_inner_size(360.0, 180.0)
                .position(x, y)
                .resizable(true)
                .maximizable(false)
                .fullscreen(false)
                .always_on_top(true)
                .focused(false)
                .build()
            {
                Ok(window) => {
                    if WINDOW_GENERATION.load(Ordering::Acquire) != generation {
                        let _ = window.close();
                        break;
                    }
                    let app = app.clone();
                    let label_for_close = label.clone();
                    let lifetime = event.lifetime;
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs_f64(lifetime)).await;
                        if let Some(window) = app.get_webview_window(&label_for_close) {
                            let _ = window.close();
                        }
                    });
                }
                Err(error) => tracing::warn!(
                    "[GlitchWindowEvent] 创建辅助窗口 {} 失败（剧本继续）: {}",
                    label,
                    error
                ),
            }
        }

        if event.interval > 0.0 && index + 1 < event.count {
            tokio::time::sleep(std::time::Duration::from_secs_f64(event.interval)).await;
        }
    }
}

/// Consume one Rust-validated ticket when its frontend queue position is reached.
pub async fn show_pending_glitch_window(
    app: &tauri::AppHandle,
    request_id: u64,
) -> Result<()> {
    let (event, generation) = take_pending_window(request_id)?;
    #[cfg(desktop)]
    create_windows(&event, app, generation).await;
    #[cfg(not(desktop))]
    {
        let _ = (event, app, generation);
        tracing::info!("[GlitchWindowEvent] 当前平台不支持辅助窗口，已安全跳过");
    }
    Ok(())
}

/// Start a new script-owned window epoch after cancelling every older run.
pub fn begin_glitch_window_run(app: &tauri::AppHandle) -> u64 {
    // Return this operation's own epoch. A concurrent later teardown may advance
    // the global value, correctly leaving the just-started run with a stale epoch.
    close_all_glitch_windows(app)
}

pub fn close_all_glitch_windows(app: &tauri::AppHandle) -> u64 {
    // Invalidate in-flight multi-window creation before closing what already exists.
    let generation = WINDOW_GENERATION
        .fetch_add(1, Ordering::AcqRel)
        .wrapping_add(1);
    clear_pending_windows();
    #[cfg(desktop)]
    {
        use tauri::Manager;
        for (label, window) in app.webview_windows() {
            if label.starts_with(WINDOW_LABEL_PREFIX) {
                let _ = window.close();
            }
        }
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
    }
    generation
}

#[async_trait]
impl ScriptEvent for GlitchWindowEvent {
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>> {
        self.validate()?;
        let allowed = {
            let game_status = ctx.game_status.lock().await;
            let status = game_status
                .script_status
                .as_ref()
                .ok_or_else(|| anyhow!("ScriptStatus 未设置，无法创建辅助窗口"))?;
            script_allows_system_effects(status)
        };
        if !allowed {
            return Err(anyhow!(
                "glitch_window 仅允许 content_warning=horror 且 script_settings.allow_system_effects=true 的剧本使用"
            ));
        }

        // Native windows used to be created here on the Rust timeline. Dialogue
        // events are paced later by the frontend, so the window could expire long
        // before the player reached this line. Queue a one-time validated ticket
        // and consume it only when the frontend event queue reaches this position.
        let request_id = queue_pending_window(self.clone(), ctx.glitch_window_generation)?;
        let payload = GlitchWindowTicketPayload {
            request_id,
            // Even a terminal-adjacent window gets one visible beat before
            // queue-ordered script:end closes it. The native lifetime continues
            // independently while later dialogue is read.
            duration: self.duration.or(Some(self.lifetime.min(0.8))),
        };
        if let Err(error) = emit(ctx.app, SCRIPT_GLITCH_WINDOW, &payload) {
            discard_pending_window(request_id);
            return Err(error);
        }
        Ok(None)
    }

    fn event_type() -> &'static str {
        "glitch_window"
    }

    fn duration(&self) -> Option<f64> {
        self.duration
    }
}

pub fn register() {
    register_event(GlitchWindowEvent::event_type(), |data| {
        Box::new(GlitchWindowEvent::from_event_data(&data))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    static WINDOW_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn clamps_window_limits() {
        let event = GlitchWindowEvent::from_event_data(&serde_json::json!({
            "count": 999,
            "lifetime": 999.0,
            "interval": 999.0
        }));
        assert_eq!(event.count, MAX_WINDOWS);
        assert_eq!(event.lifetime, MAX_LIFETIME_SECONDS);
        assert_eq!(event.interval, MAX_INTERVAL_SECONDS);
    }

    #[test]
    fn rejects_arbitrary_styles_and_oversized_text() {
        let arbitrary = GlitchWindowEvent::from_event_data(&serde_json::json!({
            "style": "https://example.invalid/payload"
        }));
        assert!(arbitrary.validate().is_err());

        let oversized = GlitchWindowEvent::from_event_data(&serde_json::json!({
            "text": "x".repeat(MAX_TEXT_CHARS + 1)
        }));
        assert!(oversized.validate().is_err());

        let control = GlitchWindowEvent::from_event_data(&serde_json::json!({
            "title": "fake\u{0000}title"
        }));
        assert!(control.validate().is_err());
    }

    #[test]
    fn validated_window_ticket_is_one_time() {
        let _guard = WINDOW_TEST_LOCK.lock().unwrap();
        clear_pending_windows();
        let event = GlitchWindowEvent::from_event_data(&serde_json::json!({
            "title": "queued",
            "text": "visible at the frontend position"
        }));
        let generation = WINDOW_GENERATION.load(Ordering::Acquire);
        let request_id = queue_pending_window(event, generation).unwrap();
        let (consumed, _) = take_pending_window(request_id).unwrap();
        assert_eq!(consumed.title, "queued");
        assert!(take_pending_window(request_id).is_err());
        clear_pending_windows();
    }

    #[test]
    fn cancelled_run_cannot_issue_a_late_ticket() {
        let _guard = WINDOW_TEST_LOCK.lock().unwrap();
        let cancelled_generation = WINDOW_GENERATION.load(Ordering::Acquire);
        WINDOW_GENERATION.fetch_add(1, Ordering::AcqRel);
        let event = GlitchWindowEvent::from_event_data(&serde_json::json!({
            "title": "too late"
        }));
        assert!(queue_pending_window(event, cancelled_generation).is_err());
    }
}
