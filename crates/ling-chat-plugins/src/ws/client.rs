//! client 连接：建连、重连退避与读写循环。

use std::sync::Arc;
use std::sync::atomic::AtomicU8;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use super::{
    PendingMap, WsCommand, WsHandle, binary_data, drain_pending, emit_conn, new_pending, set_state,
    take_pending, text_data,
};
use crate::types::{WsMode, WsState};

/// 重连退避的起始值与上限。
const RECONNECT_BACKOFF_START: Duration = Duration::from_millis(500);
const RECONNECT_BACKOFF_MAX: Duration = Duration::from_secs(30);

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
    let pending = new_pending();
    let pend = pending.clone();

    tauri::async_runtime::spawn(async move {
        let mut backoff = RECONNECT_BACKOFF_START;
        let final_state = loop {
            match build_and_connect(&target).await {
                Ok(ws) => {
                    backoff = RECONNECT_BACKOFF_START;
                    set_state(&st, WsState::Connected);
                    emit_conn(&plugin_id, &conn_id, WsMode::Client, "connect", None).await;
                    let exit = run_client(ws, &mut rx, &plugin_id, &conn_id, &pend).await;
                    // 连接没了，等待中的请求不可能再被兑现——立刻让它们失败。
                    drain_pending(&pend);
                    match exit {
                        ClientExit::Closed => break WsState::Stopped,
                        ClientExit::Disconnected => {
                            emit_conn(&plugin_id, &conn_id, WsMode::Client, "disconnect", None)
                                .await;
                            if !target.auto_reconnect {
                                break WsState::Stopped;
                            }
                            set_state(&st, WsState::Connecting);
                        },
                        ClientExit::Error(e) => {
                            emit_conn(&plugin_id, &conn_id, WsMode::Client, "error", Some(e)).await;
                            if !target.auto_reconnect {
                                break WsState::Error;
                            }
                            set_state(&st, WsState::Connecting);
                        },
                    }
                },
                Err(e) => {
                    emit_conn(&plugin_id, &conn_id, WsMode::Client, "error", Some(e)).await;
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

    WsHandle { tx, state, pending }
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
    pending: &PendingMap,
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
                // 请求帧与 Text 一样只写出，**绝不在这里等响应**：一 await 就会堵死
                // 同一个 select! 的收帧分支，变成自己等自己。
                Some(WsCommand::Request { payload, .. }) => {
                    if write.send(Message::Text(payload.into())).await.is_err() {
                        return ClientExit::Error("发送请求帧失败".into());
                    }
                },
                Some(WsCommand::Close) | None => {
                    let _ = write.close().await;
                    return ClientExit::Closed;
                },
            },
            msg = read.next() => match msg {
                Some(Ok(Message::Text(t))) => {
                    // 先给等待中的请求一次机会；命中就不再派发 ws_message。
                    if !take_pending(pending, t.as_str()) {
                        super::emit(plugin_id, conn_id, WsMode::Client, "message", Some(text_data(t.as_str())), false, None).await;
                    }
                },
                Some(Ok(Message::Binary(b))) => {
                    super::emit(plugin_id, conn_id, WsMode::Client, "message", Some(binary_data(&b)), true, None).await;
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
