//! Authenticated loopback API for deterministic memory validation.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Notify, oneshot};

use super::harness::{validate_late_autosave, validate_real};
use super::scenarios;
use super::scripted_provider::ScriptedProvider;
use crate::ai_service::memory::MemorySectionLimits;
use crate::ai_service::types::{GameLine, GameMemoryBank};

#[derive(Clone)]
pub struct ApiState {
    pub token: Arc<str>,
    pub shutdown: Arc<std::sync::Mutex<Option<oneshot::Sender<()>>>>,
    pub busy: Arc<std::sync::Mutex<bool>>,
    pub closing: Arc<std::sync::atomic::AtomicBool>,
    pub idle: Arc<Notify>,
}

struct BusyGuard {
    busy: Arc<std::sync::Mutex<bool>>,
    idle: Arc<Notify>,
}
impl Drop for BusyGuard {
    fn drop(&mut self) {
        if let Ok(mut busy) = self.busy.lock() {
            *busy = false;
        }
        self.idle.notify_waiters();
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ValidateRequest {
    #[serde(default)]
    pub scenario: String,
    #[serde(default = "default_role_id")]
    pub role_id: i32,
    #[serde(default = "default_display_name")]
    pub display_name: String,
    #[serde(default)]
    pub initial_bank: GameMemoryBank,
    /// Explicit canonical history. `line_count` remains a compact fixture shortcut.
    #[serde(default)]
    pub lines: Vec<GameLine>,
    #[serde(default = "default_line_count")]
    pub line_count: usize,
    #[serde(default = "default_update_interval")]
    pub update_interval: usize,
    #[serde(default)]
    pub recent_window: usize,
    #[serde(default)]
    pub section_limits: Option<MemorySectionLimitsRequest>,
    #[serde(default)]
    pub delay_ms: u64,
    #[serde(default)]
    pub fail_section: Option<String>,
    #[serde(default)]
    pub empty_section: Option<String>,
    #[serde(default)]
    pub panic_section: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub append_during_update: bool,
    #[serde(default)]
    pub rollback_during_update: bool,
    #[serde(default)]
    pub persistence_roundtrip: bool,
    #[serde(default)]
    pub wait_for_completion: bool,
    #[serde(default)]
    pub database: bool,
}

#[derive(Clone, Copy, Debug, Deserialize)]
pub struct MemorySectionLimitsRequest {
    #[serde(default)]
    pub short_term: usize,
    #[serde(default)]
    pub long_term: usize,
    #[serde(default)]
    pub user_info: usize,
    #[serde(default)]
    pub promises: usize,
}

impl From<Option<MemorySectionLimitsRequest>> for MemorySectionLimits {
    fn from(value: Option<MemorySectionLimitsRequest>) -> Self {
        value.map_or_else(Self::default, |limits| Self {
            short_term: limits.short_term,
            long_term: limits.long_term,
            user_info: limits.user_info,
            promises: limits.promises,
        })
    }
}

fn default_role_id() -> i32 {
    7
}
fn default_display_name() -> String {
    "Test AI".into()
}
fn default_line_count() -> usize {
    4
}
fn default_update_interval() -> usize {
    1
}
fn default_timeout_ms() -> u64 {
    10_000
}

#[derive(Clone, Debug, Serialize)]
pub struct ValidateResponse {
    pub outcome: String,
    pub scenario: String,
    pub triggered: bool,
    pub committed: bool,
    pub calls: usize,
    pub bank: GameMemoryBank,
    pub last_processed_global_idx: i64,
    pub first_processed_global_idx: i64,
    pub second_batch_committed: bool,
    pub unprocessed_tail_lines: usize,
    pub updating: bool,
    pub persistence_roundtrip: Option<bool>,
    pub persistence_result: Option<String>,
    pub system_memory: String,
    pub short_term_memory: String,
    pub duration_ms: u128,
    pub error_code: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct HealthResponse {
    ok: bool,
    busy: bool,
    mode: &'static str,
    api_version: u32,
}

fn authorized(headers: &HeaderMap, token: &str) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|candidate| candidate == token)
}
fn unauthorized() -> impl IntoResponse {
    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error":"unauthorized"})),
    )
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/memory/validate", post(validate))
        .route("/v1/scenarios/:name", post(validate_scenario))
        .route("/shutdown", post(shutdown))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .with_state(state)
}

async fn health(State(state): State<ApiState>, headers: HeaderMap) -> impl IntoResponse {
    if !authorized(&headers, &state.token) {
        return unauthorized().into_response();
    }
    let busy = state.busy.lock().map(|g| *g).unwrap_or(true);
    Json(HealthResponse {
        ok: true,
        busy,
        mode: "scripted",
        api_version: 1,
    })
    .into_response()
}

async fn validate_scenario(
    State(state): State<ApiState>,
    headers: HeaderMap,
    axum::extract::Path(name): axum::extract::Path<String>,
    body: Bytes,
) -> impl IntoResponse {
    validate_inner(state, headers, body, Some(name)).await
}

async fn validate(
    State(state): State<ApiState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    validate_inner(state, headers, body, None).await
}

async fn validate_inner(
    state: ApiState,
    headers: HeaderMap,
    body: Bytes,
    route_scenario: Option<String>,
) -> axum::response::Response {
    if !authorized(&headers, &state.token) {
        return unauthorized().into_response();
    }
    let mut request: ValidateRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error":"invalid_json"})),
            )
                .into_response();
        },
    };
    if let Some(scenario) = route_scenario {
        request.scenario = scenario;
    }
    if !scenarios::known(&request.scenario) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"unknown_scenario"})),
        )
            .into_response();
    }
    // Built-in scenarios own their inputs. Request fields must not be able to
    // turn a failure case into a successful run (or vice versa).
    match request.scenario.as_str() {
        "basic-compression" => {
            request.initial_bank = GameMemoryBank::default();
            request.fail_section = None;
            request.empty_section = None;
            request.panic_section = None;
            request.append_during_update = false;
            request.rollback_during_update = false;
            request.persistence_roundtrip = false;
            request.database = false;
            request.line_count = 4;
            request.update_interval = 1;
        },
        "append-during-update" => {
            request.initial_bank = GameMemoryBank::default();
            request.fail_section = None;
            request.empty_section = None;
            request.panic_section = None;
            request.append_during_update = true;
            request.rollback_during_update = false;
            request.line_count = 4;
            request.update_interval = 1;
            request.delay_ms = request.delay_ms.max(10);
        },
        "one-section-fails" => {
            request.initial_bank = GameMemoryBank::default();
            request.fail_section = Some("promises".into());
            request.empty_section = None;
            request.panic_section = None;
            request.append_during_update = false;
            request.rollback_during_update = false;
            request.line_count = 4;
            request.update_interval = 1;
        },
        "empty-section-fails" => {
            request.initial_bank = GameMemoryBank::default();
            request.fail_section = None;
            request.empty_section = Some("promises".into());
            request.panic_section = None;
            request.append_during_update = false;
            request.rollback_during_update = false;
            request.line_count = 4;
            request.update_interval = 1;
        },
        "panic-compression" => {
            request.initial_bank = GameMemoryBank::default();
            request.fail_section = None;
            request.empty_section = None;
            request.panic_section = Some("promises".into());
            request.append_during_update = false;
            request.rollback_during_update = false;
            request.line_count = 4;
            request.update_interval = 1;
        },
        "stale-on-rollback" => {
            request.initial_bank = GameMemoryBank::default();
            request.fail_section = None;
            request.empty_section = None;
            request.panic_section = None;
            request.append_during_update = false;
            request.rollback_during_update = true;
            request.line_count = 4;
            request.update_interval = 1;
        },
        "persistence-roundtrip" => {
            request.initial_bank = GameMemoryBank::default();
            request.fail_section = None;
            request.empty_section = None;
            request.panic_section = None;
            request.persistence_roundtrip = true;
            request.database = true;
            request.line_count = 4;
            request.update_interval = 1;
        },
        "memory-finishes-after-line-save" => {
            request.initial_bank = GameMemoryBank::default();
            request.line_count = 2;
            request.update_interval = 1;
            request.delay_ms = request.delay_ms.max(10);
            request.fail_section = None;
            request.empty_section = None;
            request.panic_section = None;
            request.append_during_update = false;
            request.rollback_during_update = false;
            request.persistence_roundtrip = false;
            request.database = false;
        },
        _ => {},
    }
    if request.role_id <= 0 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"invalid_role_id"})),
        )
            .into_response();
    }
    if request.line_count > 10_000 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"too_many_lines"})),
        )
            .into_response();
    }
    if request.update_interval == 0
        || request.timeout_ms == 0
        || request.timeout_ms > 120_000
        || request.delay_ms > 60_000
    {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"invalid_limits"})),
        )
            .into_response();
    }
    if state.closing.load(std::sync::atomic::Ordering::Acquire) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":"validation_shutting_down"})),
        )
            .into_response();
    }
    let acquired = match state.busy.lock() {
        Ok(mut busy) if !*busy && !state.closing.load(std::sync::atomic::Ordering::Acquire) => {
            *busy = true;
            true
        },
        Ok(_) => false,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error":"busy_state_unavailable"})),
            )
                .into_response();
        },
    };
    if !acquired {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"error":"validation_busy"})),
        )
            .into_response();
    }
    let _busy_guard = BusyGuard {
        busy: state.busy.clone(),
        idle: state.idle.clone(),
    };
    if request.lines.len() > 10_000 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({"error":"too_many_lines"})),
        )
            .into_response();
    }
    let provider = ScriptedProvider {
        delay_ms: request.delay_ms,
        fail_section: request.fail_section.clone(),
        empty_section: request.empty_section.clone(),
        panic_section: request.panic_section.clone(),
        ..Default::default()
    };
    let timeout = Duration::from_millis(request.timeout_ms);
    let started = std::time::Instant::now();
    if request.scenario == "memory-finishes-after-line-save" {
        let result = validate_late_autosave(
            ScriptedProvider {
                delay_ms: request.delay_ms,
                ..Default::default()
            },
            request.role_id,
            &request.display_name,
            timeout,
        )
        .await;
        return match result {
            Ok(details) => Json(serde_json::json!({
                "outcome": "succeeded",
                "scenario": request.scenario,
                "triggered": true,
                "committed": true,
                "calls": details["calls"],
                "persistence_roundtrip": true,
                "persistence_result": "saved_and_loaded",
                "details": details,
                "duration_ms": started.elapsed().as_millis(),
                "error_code": null,
            }))
            .into_response(),
            Err(error) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(serde_json::json!({
                    "error": "scenario_assertion_failed",
                    "scenario": request.scenario,
                    "outcome": "persistence_failed",
                    "detail": error.to_string(),
                })),
            )
                .into_response(),
        };
    }
    let lines = if request.lines.is_empty() {
        None
    } else {
        Some(request.lines)
    };
    // `validate_real` owns the sole timeout and aborts/joins the production
    // compression task before returning. Do not wrap it in another timeout:
    // that would release this handler while detached work is still alive.
    let result = validate_real(
        provider.clone(),
        request.initial_bank,
        request.role_id,
        lines,
        request.line_count,
        request.update_interval,
        request.recent_window,
        request.section_limits.into(),
        timeout,
        request.append_during_update,
        request.rollback_during_update,
        &request.display_name,
    )
    .await;
    let duration_ms = started.elapsed().as_millis();
    match result {
        Ok(result) => {
            let mut persistence_roundtrip = None;
            let mut persistence_error = None;
            if (request.persistence_roundtrip || request.database) && result.committed {
                match super::temp_db::TemporaryDatabase::open().await {
                    Ok(db) => match db
                        .seed_save_role(request.role_id, &request.display_name)
                        .await
                    {
                        Ok((save_id, role_id)) => {
                            match db.round_trip(save_id, role_id, &result.bank).await {
                                Ok(loaded) if loaded == result.bank => {
                                    persistence_roundtrip = Some(true)
                                },
                                Ok(_) => {
                                    persistence_roundtrip = Some(false);
                                    persistence_error = Some("round-trip bank mismatch".into())
                                },
                                Err(error) => {
                                    persistence_roundtrip = Some(false);
                                    persistence_error = Some(error.to_string())
                                },
                            }
                        },
                        Err(error) => {
                            persistence_roundtrip = Some(false);
                            persistence_error = Some(error.to_string())
                        },
                    },
                    Err(error) => {
                        persistence_roundtrip = Some(false);
                        persistence_error = Some(error.to_string())
                    },
                }
            }
            let outcome = if persistence_error.is_some() {
                "persistence_failed"
            } else if result.committed {
                "succeeded"
            } else {
                "not_committed"
            };
            // A built-in scenario is an executable contract. Never return a
            // green HTTP response when its production-derived result violates
            // the contract, and never manufacture outcome from request flags.
            let valid = match request.scenario.as_str() {
                "basic-compression" => result.triggered && result.committed && result.calls == 4,
                "append-during-update" => {
                    result.triggered
                        && result.committed
                        && result.first_processed_idx == 4
                        && result.tail_lines == 1
                        && result.second_batch_committed
                        && result.calls == 8
                },
                "one-section-fails" | "empty-section-fails" => {
                    result.triggered
                        && !result.committed
                        && result.processed_idx == 0
                        && result.calls == 4
                        && result.bank == GameMemoryBank::default()
                },
                // This contract is deliberately separate from ordinary section
                // failures: the built-in request above forces the provider to
                // panic in the promises call, while the four-call/rollback
                // assertions prove the panic was contained by the production
                // task boundary and did not commit a partial bank.
                "panic-compression" => {
                    result.triggered
                        && !result.committed
                        && result.processed_idx == 0
                        && result.calls == 4
                        && result.bank == GameMemoryBank::default()
                },
                "stale-on-rollback" => {
                    result.triggered
                        && !result.committed
                        && result.processed_idx == 0
                        && result.calls == 4
                        && result.bank == GameMemoryBank::default()
                },
                "persistence-roundtrip" => {
                    result.triggered && result.committed && persistence_roundtrip == Some(true)
                },
                _ => true,
            };
            if !valid {
                return (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(serde_json::json!({
                        "error": "scenario_assertion_failed",
                        "scenario": request.scenario,
                        "outcome": outcome,
                        "committed": result.committed,
                        "calls": result.calls,
                        "last_processed_global_idx": result.processed_idx
                    })),
                )
                    .into_response();
            }
            Json(ValidateResponse {
                outcome: outcome.into(),
                scenario: request.scenario,
                triggered: result.triggered,
                committed: result.committed,
                calls: result.calls,
                bank: result.bank,
                last_processed_global_idx: result.processed_idx,
                first_processed_global_idx: result.first_processed_idx,
                second_batch_committed: result.second_batch_committed,
                unprocessed_tail_lines: result.tail_lines,
                updating: result.updating,
                persistence_roundtrip,
                persistence_result: persistence_roundtrip.map(|ok| {
                    if ok {
                        "saved_and_loaded".into()
                    } else {
                        "failed".into()
                    }
                }),
                system_memory: result.system_memory,
                short_term_memory: result.short_term_memory,
                duration_ms,
                error_code: if result.committed {
                    persistence_error.map(|_| "persistence_failed".into())
                } else {
                    Some("compression_failed".into())
                },
            })
            .into_response()
        },
        Err(error) => {
            // The harness has already joined/aborted all production work. Keep
            // timeout as a single, truthful HTTP outcome rather than returning
            // a misleading generic validation success/failure.
            let timed_out = error.to_string().contains("timed out");
            (
                if timed_out {
                    StatusCode::REQUEST_TIMEOUT
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                },
                Json(serde_json::json!({
                    "error": if timed_out { "timed_out" } else { "validation_failed" },
                    "outcome": if timed_out { "timed_out" } else { "validation_failed" },
                    "detail": error.to_string()
                })),
            )
                .into_response()
        },
    }
}

async fn shutdown(State(state): State<ApiState>, headers: HeaderMap) -> impl IntoResponse {
    if !authorized(&headers, &state.token) {
        return unauthorized().into_response();
    }
    state
        .closing
        .store(true, std::sync::atomic::Ordering::Release);
    // Stop new validation requests first, then wait for the current handler's
    // cleanup. The handler owns and joins its production compression task on
    // timeout/error, so idle here is a real lifecycle boundary.
    loop {
        let busy = state.busy.lock().map(|guard| *guard).unwrap_or(true);
        if !busy {
            break;
        }
        let notified = state.idle.notified();
        // Keep all std mutex guards in a separate statement; none may live
        // across this await in an axum handler.
        notified.await;
    }
    let sender = state
        .shutdown
        .lock()
        .ok()
        .and_then(|mut guard| guard.take());
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
    Json(serde_json::json!({"ok":true})).into_response()
}

pub async fn serve(token: Arc<str>) -> std::io::Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let state = ApiState {
        token,
        shutdown: Arc::new(std::sync::Mutex::new(Some(shutdown_tx))),
        busy: Arc::new(std::sync::Mutex::new(false)),
        closing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        idle: Arc::new(Notify::new()),
    };
    println!(
        "{}",
        serde_json::json!({"event":"ready","host":"127.0.0.1","port":address.port(),"token":state.token,"api_version":1})
    );
    use std::io::Write;
    let _ = std::io::stdout().flush();
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await
}
