//! 插件 WebSocket 连接：常驻宿主的 client / server 连接，收帧转成 `ws_message` 信号。
//!
//! 插件脚本无状态、跑完即弃，长连接不能活在 Python 侧——连接本体与读帧循环都在
//! 这里常驻，插件只做「发一帧」（[`WsHandle`] 的发送端）与「收帧时被叫醒执行脚本」
//! （收帧组装成 payload 后走 [`PluginManager::dispatch_signal`]，与 `ai_reply` 同一条
//! 派发管线）。
//!
//! [`PluginManager::dispatch_signal`]: super::manager::PluginManager::dispatch_signal
//!
//! 连接建立/关闭都不需要 `AppHandle`（后台任务内用全局 `app_handle()` 取），
//! 因此生命周期可以完全挂在 `PluginManager` 的启停路径上。

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use axum::{
    Router,
    extract::{
        State as AxumState,
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tauri::Manager;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::AppState;

use super::signal::SIGNAL_WS_MESSAGE;
use super::types::{WsMode, WsState};

/// 重连退避的起始值与上限。
const RECONNECT_BACKOFF_START: Duration = Duration::from_millis(500);
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);

/// 发给某个连接的指令（宿主 → 连接的任务）。
///
/// `Clone` 是 server 广播通道（`broadcast::Sender`）的要求。
#[derive(Clone)]
pub(crate) enum WsCommand {
    /// 文本帧（JSON 等）。
    Text(String),
    /// 二进制帧。
    Binary(Vec<u8>),
    /// 关闭连接（client）或停止监听（server）。
    Close,
}

/// 一条活连接的句柄（发送端 + 状态）。client / server 归一到同一个类型。
#[derive(Clone)]
pub(crate) struct WsHandle {
    pub(crate) tx: mpsc::UnboundedSender<WsCommand>,
    /// 当前运行状态，由连接的后台任务更新，前端读取。
    state: Arc<AtomicU8>,
}

impl WsHandle {
    /// 当前运行状态（供插件页展示）。
    pub(crate) fn state(&self) -> WsState {
        WsState::from_u8(self.state.load(Ordering::Relaxed))
    }
}

/// 更新连接状态（后台任务写，前端读）。
fn set_state(state: &Arc<AtomicU8>, s: WsState) {
    state.store(s.as_u8(), Ordering::Relaxed);
}

/// client 连接的目标参数（占位符已解析）。
pub(crate) struct ClientTarget {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub auto_reconnect: bool,
}

/// 建立一条 client 连接（后台任务），立即返回句柄。连接失败会 emit `error` 事件。
pub(crate) fn spawn_client(plugin_id: String, conn_id: String, target: ClientTarget) -> WsHandle {
    let (tx, mut rx) = mpsc::unbounded_channel::<WsCommand>();
    let state = Arc::new(AtomicU8::new(WsState::Connecting.as_u8()));
    let st = state.clone();

    tauri::async_runtime::spawn(async move {
        let mut backoff = RECONNECT_BACKOFF_START;
        let final_state = loop {
            match build_and_connect(&target).await {
                Ok(ws) => {
                    backoff = RECONNECT_BACKOFF_START;
                    set_state(&st, WsState::Connected);
                    emit(
                        &plugin_id,
                        &conn_id,
                        WsMode::Client,
                        "connect",
                        None,
                        false,
                        None,
                    )
                    .await;
                    match run_client(ws, &mut rx, &plugin_id, &conn_id).await {
                        ClientExit::Closed => break WsState::Stopped,
                        ClientExit::Disconnected => {
                            emit(
                                &plugin_id,
                                &conn_id,
                                WsMode::Client,
                                "disconnect",
                                None,
                                false,
                                None,
                            )
                            .await;
                            if !target.auto_reconnect {
                                break WsState::Stopped;
                            }
                            set_state(&st, WsState::Connecting);
                        },
                        ClientExit::Error(e) => {
                            emit(
                                &plugin_id,
                                &conn_id,
                                WsMode::Client,
                                "error",
                                None,
                                false,
                                Some(e),
                            )
                            .await;
                            if !target.auto_reconnect {
                                break WsState::Error;
                            }
                            set_state(&st, WsState::Connecting);
                        },
                    }
                },
                Err(e) => {
                    emit(
                        &plugin_id,
                        &conn_id,
                        WsMode::Client,
                        "error",
                        None,
                        false,
                        Some(e),
                    )
                    .await;
                    if !target.auto_reconnect {
                        break WsState::Error;
                    }
                    // 会自动重连：保持 Connecting，不闪一下 Error
                },
            }

            // 退避期间若收到 Close（或句柄被丢弃）就退出；其余命令丢弃（此刻没有连接可发）。
            let wait = backoff;
            backoff = (backoff * 2).min(RECONNECT_BACKOFF_MAX);
            tokio::select! {
                _ = tokio::time::sleep(wait) => {},
                cmd = rx.recv() => {
                    if matches!(cmd, Some(WsCommand::Close) | None) {
                        break WsState::Stopped;
                    }
                },
            }
        };
        set_state(&st, final_state);
        tracing::debug!(plugin = %plugin_id, conn = %conn_id, "client 连接任务结束");
    });

    WsHandle { tx, state }
}

/// 启动一个 server 监听（后台任务），立即返回句柄。监听失败会 emit `error` 事件。
pub(crate) fn spawn_server(
    plugin_id: String,
    conn_id: String,
    bind: String,
    path: String,
) -> WsHandle {
    let (tx, mut rx) = mpsc::unbounded_channel::<WsCommand>();
    let (bc_tx, _) = broadcast::channel::<WsCommand>(32);
    let state = Arc::new(AtomicU8::new(WsState::Connecting.as_u8()));
    let st = state.clone();

    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind(&bind).await {
            Ok(l) => l,
            Err(e) => {
                emit(
                    &plugin_id,
                    &conn_id,
                    WsMode::Server,
                    "error",
                    None,
                    false,
                    Some(format!("监听 {bind} 失败: {e}")),
                )
                .await;
                set_state(&st, WsState::Error);
                return;
            },
        };
        let state = ServerState {
            plugin_id: Arc::new(plugin_id.clone()),
            conn_id: Arc::new(conn_id.clone()),
            bc_tx: bc_tx.clone(),
        };
        let router = Router::new()
            .route(&path, get(ws_upgrade))
            .with_state(state);

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        // 出站命令 → 广播给所有已连入客户端；收到 Close 则关停服务。
        let bc = bc_tx.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(cmd) = rx.recv().await {
                if matches!(cmd, WsCommand::Close) {
                    // 先广播 Close 让各连入连接 break，再停 axum——否则 graceful
                    // shutdown 会一直等这些长连接自己结束。
                    let _ = bc.send(WsCommand::Close);
                    break;
                }
                let _ = bc.send(cmd);
            }
            // Close 命令、或发送端被丢弃，都会走到这里统一触发服务关停。
            let _ = shutdown_tx.send(());
        });

        set_state(&st, WsState::Connected);
        emit(
            &plugin_id,
            &conn_id,
            WsMode::Server,
            "connect",
            None,
            false,
            None,
        )
        .await;
        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            emit(
                &plugin_id,
                &conn_id,
                WsMode::Server,
                "error",
                None,
                false,
                Some(format!("服务异常: {e}")),
            )
            .await;
        }
        emit(
            &plugin_id,
            &conn_id,
            WsMode::Server,
            "disconnect",
            None,
            false,
            None,
        )
        .await;
        set_state(&st, WsState::Stopped);
        tracing::debug!(plugin = %plugin_id, conn = %conn_id, "server 监听任务结束");
    });

    WsHandle { tx, state }
}

/// 连接建立的中间结果：成功拿到流，或失败原因。
async fn build_and_connect(
    target: &ClientTarget,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>, String> {
    let mut request = target
        .url
        .as_str()
        .into_client_request()
        .map_err(|e| format!("构建 WebSocket 请求失败: {e}"))?;
    for (k, v) in &target.headers {
        let name = http::header::HeaderName::from_bytes(k.as_bytes())
            .map_err(|e| format!("请求头名 '{k}' 非法: {e}"))?;
        let value = v
            .parse()
            .map_err(|e| format!("请求头 '{k}' 的值非法: {e}"))?;
        request.headers_mut().insert(name, value);
    }
    let (ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .map_err(|e| format!("WebSocket 连接失败: {e}"))?;
    Ok(ws)
}

/// client 读循环的退出原因。
enum ClientExit {
    /// 收到 Close 命令（正常关闭，不再重连）。
    Closed,
    /// 对端断开（可重连）。
    Disconnected,
    /// 出错（可重连）。
    Error(String),
}

/// client 读写主循环：同时 poll 命令通道（发帧）与对端帧（收帧派发）。
async fn run_client<S>(
    ws: WebSocketStream<S>,
    rx: &mut mpsc::UnboundedReceiver<WsCommand>,
    plugin_id: &str,
    conn_id: &str,
) -> ClientExit
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (mut write, mut read) = ws.split();
    loop {
        tokio::select! {
            cmd = rx.recv() => match cmd {
                Some(WsCommand::Text(t)) => {
                    if write.send(Message::Text(t.into())).await.is_err() {
                        return ClientExit::Error("发送文本帧失败".into());
                    }
                },
                Some(WsCommand::Binary(b)) => {
                    if write.send(Message::Binary(b.into())).await.is_err() {
                        return ClientExit::Error("发送二进制帧失败".into());
                    }
                },
                Some(WsCommand::Close) | None => {
                    let _ = write.close().await;
                    return ClientExit::Closed;
                },
            },
            msg = read.next() => match msg {
                Some(Ok(Message::Text(t))) => {
                    emit(plugin_id, conn_id, WsMode::Client, "message", Some(text_data(t.as_str())), false, None).await;
                },
                Some(Ok(Message::Binary(b))) => {
                    emit(plugin_id, conn_id, WsMode::Client, "message", Some(binary_data(&b)), true, None).await;
                },
                Some(Ok(Message::Ping(p))) => {
                    let _ = write.send(Message::Pong(p)).await;
                },
                Some(Ok(Message::Close(_))) => return ClientExit::Disconnected,
                Some(Err(e)) => return ClientExit::Error(e.to_string()),
                None => return ClientExit::Disconnected,
                _ => {},
            },
        }
    }
}

/// server 的 axum 共享状态。
#[derive(Clone)]
struct ServerState {
    plugin_id: Arc<String>,
    conn_id: Arc<String>,
    bc_tx: broadcast::Sender<WsCommand>,
}

/// axum 升级入口。
async fn ws_upgrade(
    ws: WebSocketUpgrade,
    AxumState(state): AxumState<ServerState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_server_ws(state, socket))
}

/// 每个连入客户端的读写循环。
async fn handle_server_ws(state: ServerState, mut socket: WebSocket) {
    let mut bc_rx = state.bc_tx.subscribe();
    let plugin_id = state.plugin_id.as_str();
    let conn_id = state.conn_id.as_str();
    loop {
        tokio::select! {
            msg = socket.next() => match msg {
                Some(Ok(AxumMessage::Text(t))) => {
                    emit(plugin_id, conn_id, WsMode::Server, "message", Some(text_data(&t)), false, None).await;
                },
                Some(Ok(AxumMessage::Binary(b))) => {
                    emit(plugin_id, conn_id, WsMode::Server, "message", Some(binary_data(&b)), true, None).await;
                },
                Some(Ok(AxumMessage::Ping(p))) => {
                    let _ = socket.send(AxumMessage::Pong(p)).await;
                },
                Some(Ok(AxumMessage::Close(_))) => break,
                Some(Err(_)) => break,
                None => break,
                _ => {},
            },
            cmd = bc_rx.recv() => match cmd {
                Ok(WsCommand::Text(t)) => {
                    let _ = socket.send(AxumMessage::Text(t.into())).await;
                },
                Ok(WsCommand::Binary(b)) => {
                    let _ = socket.send(AxumMessage::Binary(b.into())).await;
                },
                Ok(WsCommand::Close) => break,
                // 广播通道关闭（服务停止）：仅保留读支路
                Err(_) => {},
            },
        }
    }
}

/// 组装事件载荷并派发给该连接所属插件的 `ws_message` 订阅者。
///
/// `mode` 为连接方向，`event` 取 `connect` / `disconnect` / `message` / `error`。
async fn emit(
    plugin_id: &str,
    conn_id: &str,
    mode: WsMode,
    event: &str,
    data: Option<Value>,
    binary: bool,
    error: Option<String>,
) {
    let Some(app) = crate::plugins::app_handle() else {
        tracing::debug!("[PluginWS] 宿主句柄未初始化，丢弃 {event} 事件");
        return;
    };
    let payload = json!({
        "connId": conn_id,
        "mode": mode.as_str(),
        "event": event,
        "data": data.unwrap_or(Value::Null),
        "binary": binary,
        "error": error,
    });
    let manager = app.state::<AppState>().data().plugin_manager.clone();
    manager
        .dispatch_signal(&app, SIGNAL_WS_MESSAGE, &payload, Some(plugin_id))
        .await;
}

/// 文本帧的数据载荷：能解析成 JSON 就给结构化值，否则原样字符串。
fn text_data(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

/// 二进制帧的数据载荷：base64（与 `read_data_file` 的编码约定一致）。
fn binary_data(bytes: &[u8]) -> Value {
    Value::String(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// 校验 client 连接的目标 url 是否在白名单内。空白名单 = 允许全网段。
pub(crate) fn check_ws_url_allowed(url: &str, allow: &[String]) -> Result<(), String> {
    if allow.is_empty() {
        return Ok(());
    }
    let url = url.trim();
    if allow.iter().any(|pat| ws_url_matches(url, pat.trim())) {
        Ok(())
    } else {
        Err(format!(
            "不允许连接该 URL（未在 manifest 的 ws_allow 中声明）: {url}"
        ))
    }
}

/// 单个模式匹配：`*` 匹配全部；否则按 [`glob_match`]（前缀 + `*` 通配）。
fn ws_url_matches(url: &str, pattern: &str) -> bool {
    if pattern.is_empty() {
        return false;
    }
    if pattern == "*" {
        return true;
    }
    glob_match(url, pattern)
}

/// 极简 glob：无 `*` 时按前缀匹配；有 `*` 时首段锚定前缀、末段锚定后缀、
/// 中间段顺序查找。不处理 `?` 与字符类。模式以 `*` 结尾时等价于纯前缀。
fn glob_match(text: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return text.starts_with(pattern);
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0usize;
    if !parts[0].is_empty() {
        if !text.starts_with(parts[0]) {
            return false;
        }
        pos = parts[0].len();
    }
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate().skip(1) {
        if part.is_empty() {
            continue;
        }
        if i == last {
            return text[pos..].ends_with(part);
        }
        match text[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    true
}

/// 把 `${config.key}` / `${env.KEY}` 占位符替换为运行期值。未命中的占位符原样保留。
pub(crate) fn resolve_placeholders(
    text: &str,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            // 没有闭合，剩余部分原样输出
            out.push_str(&rest[start..]);
            return out;
        };
        let key = &after[..end];
        match lookup_placeholder(key, config, env) {
            Some(v) => out.push_str(&v),
            None => {
                out.push_str("${");
                out.push_str(key);
                out.push('}');
            },
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// 解析占位符键（`config.x` / `env.X`）。
fn lookup_placeholder(
    key: &str,
    config: &HashMap<String, Value>,
    env: &HashMap<String, String>,
) -> Option<String> {
    if let Some(k) = key.strip_prefix("config.") {
        config.get(k).map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
    } else if let Some(k) = key.strip_prefix("env.") {
        env.get(k).cloned()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_allow_allows_any() {
        assert!(check_ws_url_allowed("wss://evil.com/x", &[]).is_ok());
    }

    #[test]
    fn prefix_and_glob_matching() {
        let allow = vec!["wss://example.com".to_string()];
        assert!(check_ws_url_allowed("wss://example.com/ws", &allow).is_ok());
        assert!(check_ws_url_allowed("wss://evil.com/ws", &allow).is_err());
    }

    #[test]
    fn wildcard_host_matching() {
        let allow = vec!["wss://*.example.com/*".to_string()];
        assert!(check_ws_url_allowed("wss://a.example.com/ws", &allow).is_ok());
        assert!(check_ws_url_allowed("wss://a.example.com.evil.com/ws", &allow).is_err());
    }

    #[test]
    fn star_allows_any() {
        let allow = vec!["*".to_string()];
        assert!(check_ws_url_allowed("ws://127.0.0.1:9000", &allow).is_ok());
    }

    #[test]
    fn resolve_config_and_env() {
        let mut config = HashMap::new();
        config.insert("token".to_string(), Value::String("abc".to_string()));
        let mut env = HashMap::new();
        env.insert("HOST".to_string(), "example.com".to_string());
        let out = resolve_placeholders(
            "wss://${env.HOST}/ws?t=${config.token}&x=${config.missing}",
            &config,
            &env,
        );
        assert_eq!(out, "wss://example.com/ws?t=abc&x=${config.missing}");
    }

    #[test]
    fn resolve_leaves_unclosed_placeholder() {
        assert_eq!(
            resolve_placeholders("a${b", &HashMap::new(), &HashMap::new()),
            "a${b"
        );
    }
}
