//! server 监听：axum 路由、广播与每个连入客户端的读写循环。

use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use axum::{
    Router,
    extract::{
        State as AxumState,
        ws::{Message as AxumMessage, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot};

use super::{WsCommand, WsHandle, binary_data, emit_conn, new_pending, set_state, text_data};
use crate::types::{WsMode, WsState};

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
    let pending = new_pending();
    let pend = pending.clone();

    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind(&bind).await {
            Ok(l) => l,
            Err(e) => {
                emit_conn(
                    &plugin_id,
                    &conn_id,
                    WsMode::Server,
                    "error",
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
                // 请求-响应只对 client 有意义：server 是广播给多个连入客户端，
                // 谁来回、回哪一条都说不清，直接让等待方失败。
                if let WsCommand::Request { request_id, .. } = &cmd {
                    pend.lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(request_id);
                    continue;
                }
                let _ = bc.send(cmd);
            }
            // Close 命令、或发送端被丢弃，都会走到这里统一触发服务关停。
            let _ = shutdown_tx.send(());
        });

        set_state(&st, WsState::Connected);
        emit_conn(&plugin_id, &conn_id, WsMode::Server, "connect", None).await;
        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
        {
            emit_conn(
                &plugin_id,
                &conn_id,
                WsMode::Server,
                "error",
                Some(format!("服务异常: {e}")),
            )
            .await;
        }
        emit_conn(&plugin_id, &conn_id, WsMode::Server, "disconnect", None).await;
        set_state(&st, WsState::Stopped);
        tracing::debug!(plugin = %plugin_id, conn = %conn_id, "server 监听任务结束");
    });

    WsHandle { tx, state, pending }
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
                    super::emit(plugin_id, conn_id, WsMode::Server, "message", Some(text_data(&t)), false, None).await;
                },
                Some(Ok(AxumMessage::Binary(b))) => {
                    super::emit(plugin_id, conn_id, WsMode::Server, "message", Some(binary_data(&b)), true, None).await;
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
                    let _ = socket.send(AxumMessage::Text(t)).await;
                },
                Ok(WsCommand::Binary(b)) => {
                    let _ = socket.send(AxumMessage::Binary(b)).await;
                },
                Ok(WsCommand::Close) => break,
                // 请求帧在转发环节就被挡下（server 无法定位应答者），不会广播到这里；
                // 保留一支仅为穷尽匹配。
                Ok(WsCommand::Request { .. }) => {},
                // 广播落后（容量小、发帧快）丢帧继续；通道关闭仅在服务停止时发生，退出。
                Err(broadcast::error::RecvError::Lagged(_)) => {},
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
}
