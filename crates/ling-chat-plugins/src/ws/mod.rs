//! 插件 WebSocket 连接：常驻宿主的 client / server 连接，收帧转成 `ws_message` 信号。
//!
//! 插件脚本无状态、跑完即弃，长连接不能活在 Python 侧——连接本体与读帧循环都在
//! 这里常驻，插件只做「发一帧」（[`WsHandle`] 的发送端）与「收帧时被叫醒执行脚本」
//! （收帧组装成 payload 后走 [`PluginManager::dispatch_signal`]，与 `ai_reply` 同一条
//! 派发管线）。
//!
//! 连接建立/关闭都不需要 `AppHandle`（后台任务内用全局 `app_handle()` 取），
//! 因此生命周期可以完全挂在 `PluginManager` 的启停路径上。
//!
//! 文件分工：`client` / `server` 各自的生命周期，`policy` 是 URL 白名单与占位符，
//! 本文件只留句柄、指令与事件派发。

mod client;
mod policy;
mod server;

pub(crate) use client::{ClientTarget, spawn_client};
pub(crate) use policy::{check_ws_url_allowed, resolve_placeholders};
pub(crate) use server::spawn_server;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use base64::Engine as _;
use serde_json::{Value, json};
use tauri::{Emitter, Manager};
use tokio::sync::{mpsc, oneshot};

use super::PluginManager;
use super::signal::SIGNAL_WS_MESSAGE;
use super::types::{WsMode, WsState};

/// 发给某个连接的指令（宿主 → 连接的任务）。
///
/// `Clone` 是 server 广播通道（`broadcast::Sender`）的要求。
#[derive(Clone)]
pub(crate) enum WsCommand {
    /// 文本帧（JSON 等）。
    Text(String),
    /// 二进制帧。
    Binary(Vec<u8>),
    /// 请求帧：与 `Text` 一样是文本帧，区别只在**等待方在 [`WsHandle::pending`] 里
    /// 挂着**，响应帧回来时按 `request_id` 兑现。
    Request { payload: String, request_id: String },
    /// 关闭连接（client）或停止监听（server）。
    Close,
}

/// 等待响应帧的请求表：`request_id` → 兑现通道。
///
/// 放在句柄上而不是 `WsCommand` 里，是因为 `WsCommand` 必须 `Clone`（server 模式要经
/// `broadcast` 广播），而 `oneshot::Sender` 不是 `Clone`。
pub(crate) type PendingMap = Arc<std::sync::Mutex<HashMap<String, oneshot::Sender<Value>>>>;

/// 建一个空的等待表。
pub(super) fn new_pending() -> PendingMap {
    Arc::new(std::sync::Mutex::new(HashMap::new()))
}

/// 一条活连接的句柄（发送端 + 状态）。client / server 归一到同一个类型。
#[derive(Clone)]
pub(crate) struct WsHandle {
    pub(crate) tx: mpsc::UnboundedSender<WsCommand>,
    /// 当前运行状态，由连接的后台任务更新，前端读取。
    state: Arc<AtomicU8>,
    /// 等待响应帧的请求表（由脚本的 `ws_request` 登记，连接任务兑现）。
    pub(crate) pending: PendingMap,
}

/// 把一帧文本交给等待它的请求。命中返回 `true`，调用方据此跳过 `ws_message` 派发
/// ——这帧是我们自己的响应，没必要再叫醒一次解释器。
pub(super) fn take_pending(pending: &PendingMap, text: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return false;
    };
    let Some(id) = value
        .get("request_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return false;
    };
    let tx = pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&id);
    match tx {
        Some(tx) => {
            // 接收端可能已经超时放弃，发送失败属正常，不算错误。
            let _ = tx.send(value);
            true
        },
        None => false,
    }
}

/// 丢弃全部等待中的请求。连接一断就调，让等待方立刻拿到错误（通道被丢弃）
/// 而不是傻等满超时。
pub(super) fn drain_pending(pending: &PendingMap) {
    pending.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

impl WsHandle {
    /// 当前运行状态（供插件页展示）。
    pub(crate) fn state(&self) -> WsState {
        WsState::from_u8(self.state.load(Ordering::Relaxed))
    }
}

/// 更新连接状态（后台任务写，前端读）。
pub(super) fn set_state(state: &Arc<AtomicU8>, s: WsState) {
    state.store(s.as_u8(), Ordering::Relaxed);
}

/// 组装事件载荷并派发给该连接所属插件的 `ws_message` 订阅者。
///
/// `mode` 为连接方向，`event` 取 `connect` / `disconnect` / `message` / `error`。
pub(super) async fn emit(
    plugin_id: &str,
    conn_id: &str,
    mode: WsMode,
    event: &str,
    data: Option<Value>,
    binary: bool,
    error: Option<String>,
) {
    let Some(app) = crate::app_handle() else {
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
    let manager = app.state::<Arc<PluginManager>>().inner().clone();
    manager
        .dispatch_signal(&app, SIGNAL_WS_MESSAGE, &payload, Some(plugin_id))
        .await;
}

/// 生命周期事件（`connect` / `disconnect` / `error`）的便捷派发：不带数据帧。
///
/// 除了给插件自己派发 `ws_message`，也向前端发一条 `plugin:ws-state`：界面据此提示
/// 「已连上游戏」这类连接变化。前端只知道插件 id 与连接 id，不参与连接管理。
pub(super) async fn emit_conn(
    plugin_id: &str,
    conn_id: &str,
    mode: WsMode,
    event: &str,
    error: Option<String>,
) {
    emit(plugin_id, conn_id, mode, event, None, false, error).await;
    let Some(app) = crate::app_handle() else {
        return;
    };
    let _ = app.emit(
        "plugin:ws-state",
        json!({ "pluginId": plugin_id, "connId": conn_id, "event": event }),
    );
}

/// 文本帧的数据载荷：能解析成 JSON 就给结构化值，否则原样字符串。
pub(super) fn text_data(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

/// 二进制帧的数据载荷：base64（与 `read_data_file` 的编码约定一致）。
pub(super) fn binary_data(bytes: &[u8]) -> Value {
    Value::String(base64::engine::general_purpose::STANDARD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frame_resolves_pending() {
        let pending = new_pending();
        let (tx, mut rx) = oneshot::channel::<Value>();
        pending.lock().unwrap().insert("r1".to_string(), tx);

        assert!(take_pending(
            &pending,
            r#"{"type":"maid_status","request_id":"r1"}"#
        ));
        let got = rx.try_recv().unwrap();
        assert_eq!(got["type"], "maid_status");
        // 兑现过的条目不能重复命中——重复命中会把后来同 id 的帧吞掉。
        assert!(!take_pending(
            &pending,
            r#"{"type":"maid_status","request_id":"r1"}"#
        ));
    }

    #[test]
    fn unrelated_frame_is_not_taken() {
        let pending = new_pending();
        let (tx, _rx) = oneshot::channel::<Value>();
        pending.lock().unwrap().insert("r1".to_string(), tx);

        // 推送帧（无 request_id）、非法 JSON、未知 id 都不该被当成响应。
        assert!(!take_pending(&pending, r#"{"type":"event","data":{}}"#));
        assert!(!take_pending(&pending, "not json"));
        assert!(!take_pending(&pending, r#"{"request_id":"nope"}"#));
    }

    #[test]
    fn drain_fails_waiters() {
        let pending = new_pending();
        let (tx, mut rx) = oneshot::channel::<Value>();
        pending.lock().unwrap().insert("r1".to_string(), tx);

        drain_pending(&pending);
        // 发送端被丢弃 → 等待方立刻拿到 Closed，而不是等满超时。
        assert!(matches!(
            rx.try_recv(),
            Err(oneshot::error::TryRecvError::Closed)
        ));
    }
}
