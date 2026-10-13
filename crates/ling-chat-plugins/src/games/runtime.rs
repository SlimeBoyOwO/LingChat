use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    routing::any,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};
#[cfg(not(any(target_os = "android", target_os = "ios")))]
use tauri::{LogicalPosition, LogicalSize, WebviewUrl, webview::WebviewBuilder};
use tokio_util::sync::CancellationToken;

use super::{LIFECYCLE, contained_file, valid_id};
use crate::PluginManager;

const SAVE_LIMIT: usize = 8 * 1024 * 1024;
const FILE_LIMIT: u64 = 32 * 1024 * 1024;
const CSP: &str = "default-src 'none'; script-src 'self'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-src 'none'; frame-ancestors 'none'; sandbox allow-scripts allow-same-origin";

#[derive(Clone)]
struct Session {
    plugin: String,
    game: String,
    label: String,
    cancel: CancellationToken,
}
static SESSIONS: OnceLock<Mutex<HashMap<String, Session>>> = OnceLock::new();
fn sessions() -> &'static Mutex<HashMap<String, Session>> {
    SESSIONS.get_or_init(Mutex::default)
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    pub locale: String,
    pub output_device: String,
    pub reduced_motion: bool,
}

struct Server {
    app: AppHandle,
    session: Session,
    token: String,
    origin: String,
    root: PathBuf,
    save: PathBuf,
    preferences: Preferences,
    storage_lock: tokio::sync::Mutex<()>,
}

pub fn close_plugin(app: &AppHandle, plugin: &str) {
    let removed: Vec<_> = {
        let mut all = sessions().lock().unwrap_or_else(|e| e.into_inner());
        let keys: Vec<_> = all
            .iter()
            .filter(|(_, s)| s.plugin == plugin)
            .map(|(key, _)| key.clone())
            .collect();
        keys.into_iter()
            .filter_map(|key| all.remove(&key))
            .collect()
    };
    for session in removed {
        session.cancel.cancel();
        let _ = app.emit_to("main", "dlc:closed", &session.label);
        if let Some(window) = app.get_webview(&session.label) {
            let _ = window.close();
        }
    }
}

pub fn close_all(app: &AppHandle) {
    let plugins: Vec<_> = sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .map(|s| s.plugin.clone())
        .collect();
    for plugin in plugins {
        close_plugin(app, &plugin);
    }
}

/// User-initiated replacement/removal waits for the SDK's save queue. A broken
/// game must be closed by the player rather than silently discarding its state.
pub async fn request_close_plugin(app: &AppHandle, plugin: &str) -> Result<(), String> {
    let active: Vec<_> = sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .filter(|s| s.plugin == plugin)
        .cloned()
        .collect();
    for session in &active {
        if let Some(window) = app.get_webview(&session.label) {
            window
                .eval("window.dispatchEvent(new Event('dlc-before-close'))")
                .map_err(|e| e.to_string())?;
        } else {
            session.cancel.cancel();
        }
    }
    for session in active {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            session.cancel.cancelled(),
        )
        .await
        .map_err(|_| {
            "Game did not finish saving. Return from the game before updating or removing it."
                .to_string()
        })?;
    }
    Ok(())
}

pub async fn request_close_all(app: &AppHandle) -> Result<(), String> {
    let plugins: Vec<_> = sessions()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .map(|s| s.plugin.clone())
        .collect();
    for plugin in plugins {
        request_close_plugin(app, &plugin).await?;
    }
    Ok(())
}

pub fn validate_save(value: &Value) -> Result<(), String> {
    let object = value.as_object().ok_or("Game save must be an object")?;
    if object.len() > 128 || object.keys().any(|k| k.len() > 128) {
        return Err("Game save has too many keys".into());
    }
    if serde_json::to_vec(value).map_err(|e| e.to_string())?.len() > SAVE_LIMIT {
        return Err("Game save exceeds 8 MiB".into());
    }
    Ok(())
}

fn write_save(path: &std::path::Path, value: &Value) -> Result<(), String> {
    use std::io::Write;
    validate_save(value)?;
    let parent = path.parent().ok_or("Missing save directory")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut temp, value).map_err(|e| e.to_string())?;
    temp.flush().map_err(|e| e.to_string())?;
    temp.as_file().sync_all().map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

/// Copy legacy storage once. Game-specific key names remain in the DLC repository.
#[tauri::command]
pub async fn game_seed_save(
    plugin_id: String,
    game_id: String,
    value: Value,
) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    let path = save_path(&plugin_id, &game_id)?;
    if !path.exists() {
        write_save(&path, &value)?;
    }
    Ok(())
}

fn save_path(plugin: &str, game: &str) -> Result<PathBuf, String> {
    if !valid_id(plugin) || !valid_id(game) {
        return Err("Invalid game identity".into());
    }
    let data = ling_chat_main::data_dir::get_data_dir();
    let root = data.join("game_saves").join(plugin);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    if !root
        .canonicalize()
        .map_err(|e| e.to_string())?
        .starts_with(data.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("Save path outside data directory".into());
    }
    Ok(root.join(format!("{game}.json")))
}

#[tauri::command]
pub async fn game_open(
    app: AppHandle,
    plugin_id: String,
    game_id: String,
    preferences: Preferences,
    bounds: GameBounds,
) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    #[cfg(any(target_os = "android", target_os = "ios"))]
    return Err("DLC_EMBED_UNSUPPORTED".into());
    let manager = app.state::<Arc<PluginManager>>().inner().clone();
    let (root, game) = manager.game(&plugin_id, &game_id).await?;
    request_close_all(&app).await?;
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let (position, size) = bounds.checked(&app)?;
    let entry = contained_file(&root, &game.entry).map_err(|e| e.to_string())?;
    let token = uuid::Uuid::new_v4().simple().to_string();
    let label = format!("dlc-{}", uuid::Uuid::new_v4().simple());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| e.to_string())?;
    let origin = format!(
        "http://{}",
        listener.local_addr().map_err(|e| e.to_string())?
    );
    let base_url = format!("{origin}/{token}/");
    let url = format!("{base_url}{}", entry.file_name().unwrap().to_string_lossy());
    let session = Session {
        plugin: plugin_id,
        game: game_id,
        label: label.clone(),
        cancel: CancellationToken::new(),
    };
    let server = Arc::new(Server {
        app: app.clone(),
        save: save_path(&session.plugin, &session.game)?,
        session: session.clone(),
        token,
        origin,
        root: entry.parent().unwrap().into(),
        preferences,
        storage_lock: tokio::sync::Mutex::new(()),
    });
    let router = Router::new()
        .route("/:token/*path", any(handle))
        .layer(DefaultBodyLimit::max(SAVE_LIMIT))
        .with_state(server);
    let shutdown = session.cancel.clone();
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await;
    });
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let navigation_prefix = base_url.clone();
        let builder = WebviewBuilder::new(
            &label,
            WebviewUrl::External(url.parse().map_err(|e| format!("{e}"))?),
        )
        .on_navigation(move |url| url.as_str().starts_with(&navigation_prefix))
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny);
        let main = app.get_window("main").ok_or("Main window unavailable")?;
        // Register before navigation can call the exit bridge.
        sessions()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(label.clone(), session.clone());
        match main.add_child(builder, position, size) {
            Ok(view) => {
                let _ = view.set_focus();
            },
            Err(error) => {
                close_plugin(&app, &session.plugin);
                return Err(error.to_string());
            },
        }
    }
    // Includes dependency-triggered/automatic disables outside the settings UI.
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::select! {
                _ = session.cancel.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                    if manager.game(&session.plugin, &session.game).await.is_err() {
                        close_plugin(&app, &session.plugin); break;
                    }
                }
            }
        }
    });
    Ok(())
}

/// CSS-pixel rectangle reserved by the trusted main UI for the child WebView.
#[derive(Clone, Deserialize)]
pub struct GameBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl GameBounds {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    fn checked(&self, app: &AppHandle) -> Result<(LogicalPosition<f64>, LogicalSize<f64>), String> {
        let window = app.get_window("main").ok_or("Main window unavailable")?;
        let size = window
            .inner_size()
            .map_err(|e| e.to_string())?
            .to_logical::<f64>(window.scale_factor().map_err(|e| e.to_string())?);
        if ![self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            || self.x < 0.0
            || self.y < 0.0
            || self.width <= 0.0
            || self.height <= 0.0
            || self.x + self.width > size.width + 1.0
            || self.y + self.height > size.height + 1.0
        {
            return Err("Invalid game bounds".into());
        }
        Ok((
            LogicalPosition::new(self.x, self.y),
            LogicalSize::new(self.width, self.height),
        ))
    }
}

#[tauri::command]
pub async fn game_close(app: AppHandle) -> Result<(), String> {
    let _operation = LIFECYCLE.lock().await;
    request_close_all(&app).await
}

#[tauri::command]
pub async fn game_resize(app: AppHandle, bounds: GameBounds) -> Result<(), String> {
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let (position, size) = bounds.checked(&app)?;
        let active: Vec<_> = sessions()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .values()
            .cloned()
            .collect();
        for session in active {
            if let Some(view) = app.get_webview(&session.label) {
                view.set_bounds(tauri::Rect {
                    position: position.into(),
                    size: size.into(),
                })
                .map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

type HttpResult = Result<Response, (StatusCode, String)>;
fn error(status: StatusCode, message: &str) -> (StatusCode, String) {
    (status, message.into())
}

fn response(mime: &str, body: impl Into<Body>) -> Response {
    Response::builder()
        .header("Content-Type", mime)
        .header("Content-Security-Policy", CSP)
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "no-referrer")
        .header("Cache-Control", "no-store")
        .body(body.into())
        .unwrap()
}

async fn handle(
    State(server): State<Arc<Server>>,
    Path((token, path)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> HttpResult {
    if token != server.token || server.session.cancel.is_cancelled() {
        return Err(error(StatusCode::NOT_FOUND, "Session expired"));
    }
    let expected_host = server.origin.trim_start_matches("http://");
    if headers.get("host").and_then(|h| h.to_str().ok()) != Some(expected_host)
        || headers
            .get("origin")
            .is_some_and(|h| h.to_str().ok() != Some(server.origin.as_str()))
        || headers
            .get("sec-fetch-site")
            .is_some_and(|h| h == "cross-site")
    {
        return Err(error(StatusCode::FORBIDDEN, "Invalid game origin"));
    }
    if path == "_host/config" && method == Method::GET {
        let mut preferences = server.preferences.clone();
        if let Ok(store) = ling_chat_main::config::settings_store(&server.app) {
            if let Some(value) = store
                .get("audio.outputDeviceId")
                .and_then(|v| v.as_str().map(str::to_owned))
            {
                preferences.output_device = value;
            }
        }
        return Ok(response(
            "application/json",
            json!({"api_version":1,"preferences":preferences}).to_string(),
        ));
    }
    if path == "_host/save" {
        let _storage = server.storage_lock.lock().await;
        if method == Method::GET {
            let bytes = if server.save.exists() {
                if std::fs::metadata(&server.save)
                    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Cannot read save"))?
                    .len()
                    > SAVE_LIMIT as u64
                {
                    return Err(error(StatusCode::PAYLOAD_TOO_LARGE, "Save too large"));
                }
                tokio::fs::read(&server.save)
                    .await
                    .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Cannot read save"))?
            } else {
                b"{}".to_vec()
            };
            return Ok(response("application/json", bytes));
        }
        if method == Method::PUT
            && headers
                .get("content-type")
                .is_some_and(|h| h == "application/json")
        {
            let value: Value = serde_json::from_slice(&body)
                .map_err(|_| error(StatusCode::BAD_REQUEST, "Invalid JSON"))?;
            write_save(&server.save, &value).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
            return Ok(response("application/json", "{}"));
        }
        return Err(error(
            StatusCode::METHOD_NOT_ALLOWED,
            "Unsupported save request",
        ));
    }
    if path == "_host/exit" && method == Method::POST {
        close_plugin(&server.app, &server.session.plugin);
        return Ok(StatusCode::NO_CONTENT.into_response());
    }
    if method != Method::GET || path.starts_with("_host/") {
        return Err(error(StatusCode::NOT_FOUND, "Not found"));
    }
    let file = contained_file(&server.root, &path)
        .map_err(|_| error(StatusCode::NOT_FOUND, "Not found"))?;
    if std::fs::metadata(&file)
        .map_err(|_| error(StatusCode::NOT_FOUND, "Not found"))?
        .len()
        > FILE_LIMIT
    {
        return Err(error(StatusCode::PAYLOAD_TOO_LARGE, "Asset exceeds 32 MiB"));
    }
    let mime = match file.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "webp" => "image/webp",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "wav" => "audio/wav",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    };
    let bytes = tokio::fs::read(file)
        .await
        .map_err(|_| error(StatusCode::NOT_FOUND, "Not found"))?;
    Ok(response(mime, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atomic_save_round_trip_and_limits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("save.json");
        write_save(&path, &json!({"song":"one"})).unwrap();
        write_save(&path, &json!({"song":"two"})).unwrap();
        assert_eq!(
            serde_json::from_slice::<Value>(&std::fs::read(path).unwrap()).unwrap()["song"],
            "two"
        );
        assert!(validate_save(&json!([])).is_err());
        assert!(validate_save(&json!({"data":"x".repeat(SAVE_LIMIT)})).is_err());
    }
}
