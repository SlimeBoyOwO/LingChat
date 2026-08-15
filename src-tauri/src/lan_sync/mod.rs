//! 局域网数据同步模块。
//!
//! 通过 mDNS + UDP 发现局域网内的其他 LingChat 设备，
//! 使用 HTTP 协议实现手动 Push/Pull 数据同步。
//!
//! 同步范围：`data/` 下除 `third_party/` 外的所有文件。
//!
//! 前端通过以下 Tauri 命令使用此模块：
//! - `lan_sync_start_server`: 启动本地 HTTP 服务
//! - `lan_sync_stop_server`: 停止本地 HTTP 服务
//! - `lan_sync_scan_peers`: 扫描局域网设备
//! - `lan_sync_plan_push`: 生成推送计划
//! - `lan_sync_execute_push`: 执行推送
//! - `lan_sync_plan_pull`: 生成拉取计划
//! - `lan_sync_execute_pull`: 执行拉取

pub mod client;
pub mod db_sync;
pub mod discovery;
pub mod manifest;
pub mod messages;
pub mod server;
pub mod staging;
pub mod sync_engine;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};
use tauri_plugin_store::StoreExt;
use tracing::{info, warn};
use uuid::Uuid;

use crate::api::data_dir;
use crate::config::{self, keys};

use self::messages::{DeviceIdentity, PeerInfo, SyncPlan, SyncResult};

// ─── 全局状态 ────────────────────────────────────────────────

/// LAN 同步全局状态。
pub struct LanSyncState {
    /// HTTP 服务是否运行中
    pub server_running: Mutex<bool>,
    /// 当前保存的同步计划
    pub current_plan: Mutex<Option<SyncPlan>>,
    /// 设备身份（延迟加载）
    pub device_identity: Mutex<Option<DeviceIdentity>>,
    /// 本次会话标识（每次启动服务重新生成）
    pub instance_id: Mutex<Option<String>>,
    /// mDNS + UDP 宣告句柄
    pub announcer: Mutex<Option<discovery::Announcer>>,
    /// 已发现的对等设备列表
    pub peers: Mutex<Vec<PeerInfo>>,
    /// 取消标志 — 在文件间隙检查，用户可中断同步
    pub cancel_flag: AtomicBool,
}

impl Default for LanSyncState {
    fn default() -> Self {
        Self {
            server_running: Mutex::new(false),
            current_plan: Mutex::new(None),
            device_identity: Mutex::new(None),
            instance_id: Mutex::new(None),
            announcer: Mutex::new(None),
            peers: Mutex::new(Vec::new()),
            cancel_flag: AtomicBool::new(false),
        }
    }
}

/// 获取当前实例标识（只读）。
fn get_instance_id(state: &LanSyncState) -> String {
    state
        .instance_id
        .lock()
        .unwrap()
        .clone()
        .expect("instance_id 应在服务启动时已设置")
}

/// 生成新的实例标识（每次启动服务时调用）。
fn generate_instance_id(state: &LanSyncState) -> String {
    let id = Uuid::new_v4().to_string();
    let mut guard = state.instance_id.lock().unwrap();
    *guard = Some(id.clone());
    id
}

// ─── 设备身份 ────────────────────────────────────────────────

/// 设备身份文件路径。
fn device_identity_path() -> PathBuf {
    data_dir().join(".lan_sync_device.json")
}

/// 获取或创建设备身份。
///
/// 首次调用时如果磁盘上不存在身份文件，则自动生成新的 UUID 和 hostname。
fn get_device_identity(state: &LanSyncState) -> Result<DeviceIdentity, String> {
    {
        let guard = state
            .device_identity
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        if let Some(ref id) = *guard {
            return Ok(id.clone());
        }
    }

    // 尝试从磁盘加载
    let path = device_identity_path();
    if path.exists() {
        match fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<DeviceIdentity>(&content) {
                Ok(id) => {
                    let mut guard = state
                        .device_identity
                        .lock()
                        .map_err(|e| format!("锁失败: {e}"))?;
                    *guard = Some(id.clone());
                    return Ok(id);
                }
                Err(e) => warn!("设备身份文件损坏，将重新生成: {e}"),
            },
            Err(e) => warn!("无法读取设备身份文件，将重新生成: {e}"),
        }
    }

    // 生成新身份
    let device_name = get_hostname();

    let identity = DeviceIdentity {
        device_id: Uuid::new_v4().to_string(),
        device_name,
    };

    // 持久化
    let json = serde_json::to_string_pretty(&identity).map_err(|e| format!("序列化失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入设备身份文件失败: {e}"))?;

    let mut guard = state
        .device_identity
        .lock()
        .map_err(|e| format!("锁失败: {e}"))?;
    *guard = Some(identity.clone());

    info!(
        "已生成设备身份: id={}, name={}",
        identity.device_id, identity.device_name
    );
    Ok(identity)
}

/// 获取 hostname（用于设备名）。
fn get_hostname() -> String {
    // 优先使用环境变量
    #[cfg(target_os = "windows")]
    {
        if let Ok(name) = std::env::var("COMPUTERNAME") {
            if !name.is_empty() {
                return name;
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(name) = std::env::var("HOSTNAME") {
            if !name.is_empty() {
                return name;
            }
        }
    }
    // 回退
    "Unknown Device".to_string()
}

// ─── 配对令牌与受信设备 ────────────────────────────────────────

/// 受信对端设备存储：`data/.lan_sync_peers.json`，`device_id → 配对令牌`。
fn trusted_peers_path() -> PathBuf {
    data_dir().join(".lan_sync_peers.json")
}

fn load_trusted_peers() -> HashMap<String, String> {
    let path = trusted_peers_path();
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(_) => return HashMap::new(),
    };
    serde_json::from_str::<HashMap<String, String>>(&content).unwrap_or_default()
}

fn save_trusted_peers(peers: &HashMap<String, String>) -> Result<(), String> {
    let path = trusted_peers_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    let json = serde_json::to_string_pretty(peers).map_err(|e| format!("序列化失败: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("写入配对信息失败: {e}"))
}

/// 查询已配对设备的令牌（客户端请求时使用）。
pub(crate) fn trusted_peer_token(device_id: &str) -> Option<String> {
    load_trusted_peers().get(device_id).cloned()
}

/// 获取或生成本机配对令牌：优先 keyring，keyring 不可用时降级 settings.json 明文。
fn get_or_create_auth_token(app: &AppHandle) -> Result<String, String> {
    let key = keys::LAN_SYNC_AUTH_TOKEN;
    if crate::security::secrets::secret_storage_available() {
        // 1) keyring 已有
        match crate::security::secrets::get_secret(key) {
            Ok(Some(token)) => return Ok(token),
            Ok(None) => {}
            Err(e) => warn!("读取 keyring 中 LAN 同步令牌失败: {e}"),
        }

        let store = app
            .store(config::STORE_FILE)
            .map_err(|e| format!("打开设置存储失败: {e}"))?;
        // 2) settings.json 明文残留 → 迁到 keyring 并清空明文
        if let Some(token) = store
            .get(key)
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
        {
            match crate::security::secrets::set_secret(key, &token) {
                Ok(()) => {
                    store.set(key.to_string(), serde_json::json!(""));
                    let _ = store.save();
                }
                Err(e) => warn!("迁移 LAN 同步令牌到 keyring 失败: {e}"),
            }
            return Ok(token);
        }

        // 3) 全新生成（keyring 失败则降级明文）
        let token = Uuid::new_v4().to_string();
        match crate::security::secrets::set_secret(key, &token) {
            Ok(()) => {}
            Err(e) => {
                warn!("keyring 不可用，LAN 同步令牌降级明文存储: {e}");
                store.set(key.to_string(), serde_json::json!(token));
                store
                    .save()
                    .map_err(|e| format!("保存设置存储失败: {e}"))?;
            }
        }
        return Ok(token);
    }

    // 无 keyring 平台（移动端）：settings.json 明文存储
    let store = app
        .store(config::STORE_FILE)
        .map_err(|e| format!("打开设置存储失败: {e}"))?;
    if let Some(token) = store
        .get(key)
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
    {
        return Ok(token);
    }
    let token = Uuid::new_v4().to_string();
    store.set(key.to_string(), serde_json::json!(token));
    store
        .save()
        .map_err(|e| format!("保存设置存储失败: {e}"))?;
    Ok(token)
}

// ─── Tauri 命令 ──────────────────────────────────────────────

/// 启动本地 HTTP 同步服务。
///
/// 默认仅绑定 127.0.0.1 回环 + 全端点 Bearer 令牌认证；用户显式开启
/// 「暴露到局域网」后才绑定全网卡（仍要求令牌）。返回实际绑定的端口号。
#[tauri::command]
pub async fn lan_sync_start_server(
    app: AppHandle,
    state: State<'_, LanSyncState>,
) -> Result<u16, String> {
    // 并发检查
    {
        let mut running = state
            .server_running
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        if *running {
            return Err("同步服务已在运行中".to_string());
        }
        *running = true;
    }

    let identity = get_device_identity(&state)?;
    let instance_id = generate_instance_id(&state);

    // 配对令牌 + 局域网暴露开关
    let auth_token = get_or_create_auth_token(&app)?;
    let expose_lan = app
        .store(config::STORE_FILE)
        .ok()
        .and_then(|store| store.get(keys::LAN_SYNC_EXPOSE_LAN))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // 启动 HTTP 服务
    let port = server::start_server(app.clone(), &identity, auth_token, expose_lan).await?;

    // 启动 mDNS 宣告 + UDP 监听
    let announcer = discovery::Announcer::start(&identity, &instance_id, port).await?;
    {
        let mut guard = state.announcer.lock().map_err(|e| format!("锁失败: {e}"))?;
        *guard = Some(announcer);
    }

    info!(
        "LAN 同步服务已启动，端口: {}, 实例: {}, 暴露局域网: {}",
        port,
        &instance_id[..8],
        expose_lan
    );
    Ok(port)
}

/// 查看本机配对令牌（供用户在另一台设备上输入完成配对）。
#[tauri::command]
pub async fn lan_sync_get_token(app: AppHandle) -> Result<String, String> {
    get_or_create_auth_token(&app)
}

/// 保存（或更新）某台对端设备的配对令牌；token 为空表示解除配对。
#[tauri::command]
pub async fn lan_sync_set_peer_token(
    device_id: String,
    token: String,
) -> Result<(), String> {
    let mut peers = load_trusted_peers();
    let trimmed = token.trim().to_string();
    if trimmed.is_empty() {
        peers.remove(&device_id);
    } else {
        peers.insert(device_id.clone(), trimmed);
    }
    save_trusted_peers(&peers)?;
    info!("已更新设备配对信息: {device_id}");
    Ok(())
}

/// 解除与某台设备的配对。
#[tauri::command]
pub async fn lan_sync_remove_peer_token(device_id: String) -> Result<(), String> {
    let mut peers = load_trusted_peers();
    peers.remove(&device_id);
    save_trusted_peers(&peers)
}

/// 查询某台设备是否已配对（返回已保存的令牌；None 表示未配对）。
#[tauri::command]
pub async fn lan_sync_get_peer_token(device_id: String) -> Result<Option<String>, String> {
    Ok(load_trusted_peers().get(&device_id).cloned())
}

/// 停止本地 HTTP 同步服务。
#[tauri::command]
pub async fn lan_sync_stop_server(state: State<'_, LanSyncState>) -> Result<(), String> {
    {
        let running = state
            .server_running
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        if !*running {
            return Err("同步服务未运行".to_string());
        }
    } // MutexGuard 在此 drop，释放锁

    // 先停止宣告（mDNS + UDP）
    {
        let mut guard = state.announcer.lock().map_err(|e| format!("锁失败: {e}"))?;
        if let Some(a) = guard.take() {
            a.stop();
        }
    }

    server::stop_server().await?;

    {
        let mut running = state
            .server_running
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *running = false;
    }

    info!("LAN 同步服务已停止");
    Ok(())
}

/// 扫描局域网中的其他 LingChat 设备。
///
/// 并行运行 mDNS 浏览 + UDP 广播探测。
/// 复用 `Announcer` 持有的 daemon，不创建新的。
/// 结果通过 `lan-sync-peers-updated` 事件推送到前端，同时直接返回。
#[tauri::command]
pub async fn lan_sync_scan_peers(
    app: AppHandle,
    state: State<'_, LanSyncState>,
) -> Result<Vec<PeerInfo>, String> {
    let instance_id = get_instance_id(&state);

    // 从 Announcer 获取共享的 daemon（clone 后释放锁）
    let daemon = {
        let guard = state.announcer.lock().map_err(|e| format!("锁失败: {e}"))?;
        guard
            .as_ref()
            .ok_or("同步服务未启动，请先打开同步面板")?
            .daemon
            .clone()
    };

    let peers = discovery::discover_peers(&app, &daemon, &instance_id).await?;

    // 更新缓存
    {
        let mut cache = state.peers.lock().map_err(|e| format!("锁失败: {e}"))?;
        *cache = peers.clone();
    }

    // 推送到前端
    let _ = app.emit("lan-sync-peers-updated", &peers);

    info!("发现 {} 个局域网设备", peers.len());
    Ok(peers)
}

/// 生成推送计划（本地 → 对端）。
///
/// 计划通过 `lan-sync-plan` 事件发送到前端供用户确认。
#[tauri::command]
pub async fn lan_sync_plan_push(
    app: AppHandle,
    state: State<'_, LanSyncState>,
    peer: PeerInfo,
) -> Result<(), String> {
    let identity = get_device_identity(&state)?;
    let plan = sync_engine::plan_push(&identity, &peer).await?;

    let _ = app.emit("lan-sync-plan", &plan);

    let mut guard = state
        .current_plan
        .lock()
        .map_err(|e| format!("锁失败: {e}"))?;
    *guard = Some(plan);

    Ok(())
}

/// 生成拉取计划（对端 → 本地）。
///
/// 计划通过 `lan-sync-plan` 事件发送到前端供用户确认。
#[tauri::command]
pub async fn lan_sync_plan_pull(
    app: AppHandle,
    state: State<'_, LanSyncState>,
    peer: PeerInfo,
) -> Result<(), String> {
    let identity = get_device_identity(&state)?;
    let plan = sync_engine::plan_pull(&identity, &peer).await?;

    let _ = app.emit("lan-sync-plan", &plan);

    let mut guard = state
        .current_plan
        .lock()
        .map_err(|e| format!("锁失败: {e}"))?;
    *guard = Some(plan);

    Ok(())
}

/// 执行已计划的推送操作。
///
/// 传输过程中通过 `lan-sync-progress` 事件报告进度，
/// 完成后通过 `lan-sync-complete` 发送结果。
#[tauri::command]
pub async fn lan_sync_execute_push(
    app: AppHandle,
    state: State<'_, LanSyncState>,
) -> Result<SyncResult, String> {
    let plan = {
        let guard = state
            .current_plan
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        guard
            .clone()
            .ok_or_else(|| "没有待执行的同步计划".to_string())?
    };

    // 重置取消标志
    state
        .cancel_flag
        .store(false, std::sync::atomic::Ordering::SeqCst);

    let result = sync_engine::execute_push(&plan, &app, &state.cancel_flag).await;

    // 发送完成事件
    match &result {
        Ok(r) => {
            let _ = app.emit("lan-sync-complete", r);
        }
        Err(e) => {
            let _ = app.emit(
                "lan-sync-complete",
                &SyncResult {
                    success: false,
                    direction: "push".to_string(),
                    files_downloaded: 0,
                    files_deleted: 0,
                    files_staged: 0,
                    bytes_transferred: 0,
                    message: e.clone(),
                },
            );
        }
    }

    // 清空计划
    {
        let mut guard = state
            .current_plan
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *guard = None;
    }

    result
}

/// 执行已计划的拉取操作。
///
/// 传输过程中通过 `lan-sync-progress` 事件报告进度，
/// 完成后通过 `lan-sync-complete` 发送结果。
#[tauri::command]
pub async fn lan_sync_execute_pull(
    app: AppHandle,
    state: State<'_, LanSyncState>,
) -> Result<SyncResult, String> {
    let plan = {
        let guard = state
            .current_plan
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        guard
            .clone()
            .ok_or_else(|| "没有待执行的同步计划".to_string())?
    };

    // 重置取消标志
    state
        .cancel_flag
        .store(false, std::sync::atomic::Ordering::SeqCst);

    let result = sync_engine::execute_pull(&plan, &app, &state.cancel_flag).await;

    // 发送完成事件
    match &result {
        Ok(r) => {
            let _ = app.emit("lan-sync-complete", r);
        }
        Err(e) => {
            let _ = app.emit(
                "lan-sync-complete",
                &SyncResult {
                    success: false,
                    direction: "pull".to_string(),
                    files_downloaded: 0,
                    files_deleted: 0,
                    files_staged: 0,
                    bytes_transferred: 0,
                    message: e.clone(),
                },
            );
        }
    }

    // 清空计划
    {
        let mut guard = state
            .current_plan
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        *guard = None;
    }

    result
}

// ─── 重启 ─────────────────────────────────────────────────────

/// 重启应用以应用暂存的同步文件（桌面端，依赖 tauri-plugin-process）。
#[cfg(desktop)]
#[tauri::command]
pub fn lan_sync_restart(app: tauri::AppHandle) -> Result<(), String> {
    app.restart();
}

/// 重启应用（移动端暂不支持，返回提示）。
#[cfg(not(desktop))]
#[tauri::command]
pub fn lan_sync_restart() -> Result<(), String> {
    Err("重启功能仅在桌面端可用".to_string())
}
