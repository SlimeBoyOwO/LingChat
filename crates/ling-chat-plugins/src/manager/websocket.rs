//! 插件声明的 WebSocket 连接表与建连参数。

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::Value;
use tokio::sync::oneshot;

use crate::python_backend;
use crate::types::{PluginRecord, WsConnInfo, WsDecl, WsState};
use crate::ws::{self, WsCommand, WsHandle};

/// id → (conn_id → 连接句柄)。与取消令牌同步维护：插件启用即建、停用即断。
///
/// 用 `std::sync::Mutex`：读写都是「查表 + 发 mpsc」的同步操作，且发送端可能在
/// 插件脚本线程里被调用（那里不能 await）。
pub(super) struct WsHub {
    table: Mutex<HashMap<String, HashMap<String, WsHandle>>>,
}

impl WsHub {
    pub(super) fn new() -> Self {
        Self {
            table: Mutex::new(HashMap::new()),
        }
    }

    /// 关闭某插件的 WS 连接（`conn_id` 为 None 时关闭全部）。
    pub(super) fn close(&self, id: &str, conn_id: Option<&str>) {
        let mut table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        match conn_id {
            None => {
                if let Some(conns) = table.remove(id) {
                    for handle in conns.values() {
                        let _ = handle.tx.send(WsCommand::Close);
                    }
                }
            },
            Some(cid) => {
                if let Some(conns) = table.get_mut(id) {
                    if let Some(handle) = conns.remove(cid) {
                        let _ = handle.tx.send(WsCommand::Close);
                    }
                }
            },
        }
    }

    /// 关闭所有插件的 WS 连接（重载时全量重建用）。
    pub(super) fn close_all(&self) {
        let mut table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        for (_, conns) in table.drain() {
            for handle in conns.values() {
                let _ = handle.tx.send(WsCommand::Close);
            }
        }
    }

    /// 向某连接发一帧（脚本 `ws_send` 用）。同步、不等待、不回调。
    pub(super) fn send(
        &self,
        plugin_id: &str,
        conn_id: &str,
        cmd: WsCommand,
    ) -> Result<(), String> {
        let table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        let conns = table
            .get(plugin_id)
            .ok_or_else(|| format!("插件 '{plugin_id}' 没有活动连接"))?;
        let handle = conns
            .get(conn_id)
            .ok_or_else(|| format!("连接 '{conn_id}' 未建立或已关闭"))?;
        handle
            .tx
            .send(cmd)
            .map_err(|_| format!("连接 '{conn_id}' 的发送通道已关闭"))
    }

    /// 向某连接发一条请求并登记等待（脚本 `ws_request` 用），返回应答通道。
    pub(super) fn request(
        &self,
        plugin_id: &str,
        conn_id: &str,
        request_id: String,
        payload: String,
    ) -> Result<oneshot::Receiver<Value>, String> {
        let table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        let conns = table
            .get(plugin_id)
            .ok_or_else(|| format!("插件 '{plugin_id}' 没有活动连接"))?;
        let handle = conns
            .get(conn_id)
            .ok_or_else(|| format!("连接 '{conn_id}' 未建立或已关闭"))?;
        let (tx, rx) = oneshot::channel();
        handle
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(request_id.clone(), tx);
        let sent = handle.tx.send(WsCommand::Request {
            payload,
            request_id: request_id.clone(),
        });
        if sent.is_err() {
            // 发送通道已关：撤掉刚登记的等待，免得留下永不兑现的条目。
            handle
                .pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&request_id);
            return Err(format!("连接 '{conn_id}' 的发送通道已关闭"));
        }
        Ok(rx)
    }

    /// 某条连接的实时状态；表里没有 = 未建立。
    pub(super) fn state_of(&self, plugin_id: &str, conn_id: &str) -> Option<WsState> {
        let table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        table
            .get(plugin_id)
            .and_then(|conns| conns.get(conn_id))
            .map(WsHandle::state)
    }

    /// 查询插件声明的连接及当前状态（脚本 `ws_status` 用）。
    ///
    /// `conn_id` 指定时只返回该连接（未声明则报错）；否则返回全部声明。
    pub(super) fn status(
        &self,
        plugin_id: &str,
        decls: &[WsDecl],
        conn_id: Option<&str>,
    ) -> Result<Vec<WsConnInfo>, String> {
        let table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        let conns = table.get(plugin_id);
        let mut out = Vec::new();
        for decl in decls {
            if conn_id.is_some_and(|c| c != decl.id) {
                continue;
            }
            // 表里没有该连接 = 未建立（Stopped）。
            let state = conns
                .and_then(|c| c.get(&decl.id))
                .map(|h| h.state())
                .unwrap_or(WsState::Stopped);
            out.push(WsConnInfo {
                id: decl.id.clone(),
                mode: decl.mode,
                state,
            });
        }
        if let Some(cid) = conn_id {
            if out.is_empty() {
                return Err(format!("插件 '{plugin_id}' 未声明连接 '{cid}'"));
            }
        }
        Ok(out)
    }

    /// 同步建立连接（只碰连接表）。`only` 指定时只处理该 conn_id，
    /// `url_override` 覆盖 client 的声明 url。
    pub(super) fn spawn(
        &self,
        id: &str,
        input: &WsOpenInput,
        only: Option<&str>,
        url_override: Option<&str>,
    ) -> Result<(), String> {
        // 无任何 ws 声明：不在连接表里留空条目（启用无 WS 能力的插件会走到这里）。
        // 脚本按 id 显式请求（only 有值）时继续往下走，在末尾报「未声明」。
        if input.decls.is_empty() && only.is_none() {
            return Ok(());
        }
        let mut table = self.table.lock().unwrap_or_else(|e| e.into_inner());
        let conns = table.entry(id.to_string()).or_default();
        let mut matched = false;
        for decl in &input.decls {
            if only.is_some_and(|c| c != decl.id) {
                continue;
            }
            matched = true;
            if conns.contains_key(&decl.id) {
                continue; // 幂等：已建立则跳过
            }
            match decl.mode {
                crate::types::WsMode::Client => {
                    let url = url_override
                        .map(str::to_string)
                        .or_else(|| decl.url.clone())
                        .map(|u| ws::resolve_placeholders(&u, &input.config, &input.env))
                        .unwrap_or_default();
                    if url.is_empty() {
                        let msg = format!("连接 '{}' 未提供 url", decl.id);
                        if only.is_some() {
                            return Err(msg);
                        }
                        tracing::warn!(plugin = %id, conn = %decl.id, "{msg}，跳过");
                        continue;
                    }
                    if let Err(e) = ws::check_ws_url_allowed(&url, &input.allow) {
                        if only.is_some() {
                            return Err(e);
                        }
                        tracing::warn!(plugin = %id, conn = %decl.id, "url 不在白名单，跳过: {e}");
                        continue;
                    }
                    let headers = decl
                        .headers
                        .iter()
                        .map(|(k, v)| {
                            (
                                k.clone(),
                                ws::resolve_placeholders(v, &input.config, &input.env),
                            )
                        })
                        .collect();
                    let handle = ws::spawn_client(
                        id.to_string(),
                        decl.id.clone(),
                        ws::ClientTarget {
                            url,
                            headers,
                            auto_reconnect: decl.auto_reconnect,
                        },
                    );
                    conns.insert(decl.id.clone(), handle);
                },
                crate::types::WsMode::Server => {
                    let bind = decl.bind.clone().unwrap_or_default();
                    let raw_path = decl.path.clone().unwrap_or_else(|| "/ws".to_string());
                    // axum 的 route 要求以 / 开头，缺了会 panic。
                    let path = if raw_path.starts_with('/') {
                        raw_path
                    } else {
                        format!("/{raw_path}")
                    };
                    let handle = ws::spawn_server(id.to_string(), decl.id.clone(), bind, path);
                    conns.insert(decl.id.clone(), handle);
                },
            }
        }
        if let Some(only) = only {
            if !matched {
                return Err(format!("插件 '{id}' 未声明连接 '{only}'"));
            }
        }
        Ok(())
    }
}

/// 建立某插件 WS 连接所需的、已从 records 取出的数据（取出后与锁无关，
/// 因此可在 async 与 blocking 两种上下文里建连）。
pub(super) struct WsOpenInput {
    pub(super) decls: Vec<WsDecl>,
    pub(super) allow: Vec<String>,
    pub(super) config: HashMap<String, Value>,
    pub(super) env: HashMap<String, String>,
}

/// 从插件记录拍出建连输入。
pub(super) fn ws_open_input(record: &PluginRecord) -> WsOpenInput {
    WsOpenInput {
        decls: record.manifest.ws.clone(),
        allow: record.manifest.ws_allow.clone(),
        config: record.state.config.clone(),
        env: python_backend::collect_env(&record.manifest),
    }
}
