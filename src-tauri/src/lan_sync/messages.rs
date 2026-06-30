//! LAN 同步用的序列化消息类型。
//!
//! 所有面向前端的类型均使用 `camelCase` 命名，与 Tauri 事件系统的约定保持一致。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::data_update::manifest::FileEntry;

// ─── 设备标识 ────────────────────────────────────────────────

/// 设备身份标识，首次同步时生成并持久化到 `data/.lan_sync_device.json`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub device_name: String,
}

// ─── 发现阶段 ────────────────────────────────────────────────

/// 局域网中发现的对等设备。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    /// 设备唯一标识（UUIDv4）
    pub device_id: String,
    /// 本次会话标识（UUIDv4，每次启动服务重新生成）
    pub instance_id: String,
    /// 人类可读的设备名（hostname）
    pub device_name: String,
    /// IPv4 地址
    pub host: String,
    /// HTTP 服务端口
    pub port: u16,
    /// 对端的数据版本号
    pub data_version: u64,
    /// 对端的文件总数
    pub file_count: u64,
}

/// UDP 广播发现消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryMessage {
    pub msg_type: String,
    pub device_id: String,
    pub instance_id: String,
    pub device_name: String,
    pub host: String,
    pub port: u16,
    pub data_version: u64,
    pub file_count: u64,
}

// ─── mDNS TXT 记录键名 ───────────────────────────────────────

pub const TXT_DEVICE_ID: &str = "id";
pub const TXT_INSTANCE_ID: &str = "iid";
pub const TXT_DEVICE_NAME: &str = "name";
pub const TXT_DATA_VERSION: &str = "ver";
pub const TXT_FILE_COUNT: &str = "cnt";

/// mDNS 服务类型
pub const MDNS_SERVICE_TYPE: &str = "_lingchat-sync._tcp.local.";

/// UDP 广播端口（固定）
pub const UDP_DISCOVERY_PORT: u16 = 9666;

// ─── 清单 ────────────────────────────────────────────────────

/// 完整清单：包含 data_manifest.json 中已追踪的文件 + 运行时动态扫描的文件。
///
/// 注意：third_party/ 目录在扫描时被完全排除。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompleteManifest {
    pub device_id: String,
    pub data_version: u64,
    /// 已在 data_manifest.json 中登记的文件（key = 相对 data/ 的路径，使用 `/`）
    pub files: HashMap<String, FileEntry>,
    /// 不在清单中但存在于磁盘上的运行时文件（如 voice/, screenshots/, game_database.db 等）
    #[serde(default)]
    pub runtime_files: HashMap<String, FileEntry>,
}

// ─── 同步计划 ────────────────────────────────────────────────

/// 同步方向。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SyncDirection {
    Pull,
    Push,
}

/// 同步操作中的单个文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFileOp {
    /// 相对 data/ 的路径
    pub path: String,
    /// SHA-256
    pub sha256: String,
    /// 文件大小（字节）
    pub size: u64,
    /// 操作原因
    pub reason: String,
}

/// 同步计划 — 执行前展示给用户确认。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    pub direction: SyncDirection,
    pub peer: PeerInfo,
    /// 需要传输的文件列表
    pub files_to_transfer: Vec<SyncFileOp>,
    /// 需要在远端/本地删除的文件列表
    pub files_to_delete: Vec<String>,
    /// 总传输字节数
    pub total_bytes: u64,
}

// ─── 数据库同步 ──────────────────────────────────────────────

/// 数据库记录导出包，用于 LAN 同步传输。
///
/// 每张表的全部行被序列化为对应 Model 的 JSON 数组。
/// 导入时按外键依赖顺序 DELETE 全部行后重新 INSERT。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbRecords {
    pub device_id: String,
    pub roles: Vec<serde_json::Value>,
    pub saves: Vec<serde_json::Value>,
    pub running_scripts: Vec<serde_json::Value>,
    pub adventure_unlocks: Vec<serde_json::Value>,
    pub lines: Vec<serde_json::Value>,
    pub memory_banks: Vec<serde_json::Value>,
    pub line_perceptions: Vec<serde_json::Value>,
}

// ─── 进度事件 ────────────────────────────────────────────────

/// 同步进度事件，通过 `lan-sync-progress` 发送到前端。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgressEvent {
    /// 当前阶段
    pub phase: String,
    /// 当前文件序号
    pub current: u64,
    /// 总文件数
    pub total: u64,
    /// 0-100 百分比
    pub progress: u32,
    /// 当前传输的文件名
    pub current_file: Option<String>,
    /// 已传输字节数
    pub bytes_transferred: u64,
    /// 附加消息
    pub message: Option<String>,
}

// ─── 结果 ────────────────────────────────────────────────────

/// 同步完成结果，通过 `lan-sync-complete` 发送到前端。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub success: bool,
    pub direction: String,
    pub files_downloaded: u64,
    pub files_deleted: u64,
    /// 因文件被锁定而暂存、需重启后生效的文件数
    pub files_staged: u64,
    pub bytes_transferred: u64,
    pub message: String,
}
