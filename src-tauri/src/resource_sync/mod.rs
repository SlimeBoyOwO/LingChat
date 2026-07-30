//! 本地资源同步模块。
//!
//! 将安装包自带的默认资源（`data/.official/`）同步到工作目录（`data/`）。
//!
//! 设计对比：
//! - `data_update`（将被删除）：从 GitHub Release 下载 → 解压 → 合并，网络依赖
//! - `resource_sync`（本模块）：从安装包本地目录复制，纯本地操作
//!
//! 前端通过两个 Tauri 命令使用：
//! - `check_resource_sync`: 比对 .official 与 data/ 的清单，返回差异
//! - `apply_resource_sync`: 将选中的文件复制到 data/

pub mod sync;

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, State};
use tracing::info;

use crate::init::static_copy::get_data_dir;
use crate::manifest::DataManifest;

// ─── 状态 ────────────────────────────────────────────────────

/// 资源同步全局状态（防止并发）。
#[derive(Default)]
pub struct ResourceSyncState {
    pub syncing: Mutex<bool>,
}

// ─── 面向前端的类型 ──────────────────────────────────────────

/// 资源同步差异信息（返回给前端展示）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSyncInfo {
    pub available: bool,
    pub new_version: u64,
    pub current_version: u64,
    pub files_to_add: Vec<SyncFileEntry>,
    pub files_to_modify: Vec<SyncFileEntry>,
    pub total_size: u64,
}

/// 单个文件的变更条目。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncFileEntry {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    /// "add" | "modify"
    pub change_type: String,
}

/// 资源同步执行结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSyncResult {
    pub success: bool,
    pub files_synced: usize,
    pub message: String,
}

// ─── Tauri 命令 ──────────────────────────────────────────────

/// 检查是否有可用的数据资源更新。
///
/// 对比 `data/.official/data_manifest.json`（安装包版本）与
/// `data/data_manifest.json`（本地工作副本），仅返回 add 和 modify。
#[tauri::command]
pub async fn check_resource_sync() -> Result<ResourceSyncInfo, String> {
    let data_dir = get_data_dir().clone();
    let official_manifest_path = data_dir.join(".official").join("data_manifest.json");
    let local_manifest_path = data_dir.join("data_manifest.json");

    // .official 不存在 → 无更新
    if !official_manifest_path.exists() {
        return Ok(ResourceSyncInfo {
            available: false,
            new_version: 0,
            current_version: 0,
            files_to_add: vec![],
            files_to_modify: vec![],
            total_size: 0,
        });
    }

    let official_manifest =
        DataManifest::load(&official_manifest_path).map_err(|e| format!("读取官方清单失败: {e}"))?;

    let local_manifest = if local_manifest_path.exists() {
        DataManifest::load(&local_manifest_path).unwrap_or(DataManifest {
            data_version: 0,
            files: HashMap::new(),
        })
    } else {
        DataManifest {
            data_version: 0,
            files: HashMap::new(),
        }
    };

    // 版本已是最新 → 清理残留 .official
    if official_manifest.data_version <= local_manifest.data_version {
        info!(
            "数据已是最新 (local={}, resource={}), 清理残留 .official",
            local_manifest.data_version, official_manifest.data_version
        );
        let _ = std::fs::remove_dir_all(data_dir.join(".official"));
        return Ok(ResourceSyncInfo {
            available: false,
            new_version: official_manifest.data_version,
            current_version: local_manifest.data_version,
            files_to_add: vec![],
            files_to_modify: vec![],
            total_size: 0,
        });
    }

    let diff = local_manifest.diff(&official_manifest);

    let files_to_add: Vec<SyncFileEntry> = diff
        .files_to_add
        .iter()
        .map(|path| {
            let entry = &official_manifest.files[path];
            SyncFileEntry {
                path: path.clone(),
                sha256: entry.sha256.clone(),
                size: entry.size,
                change_type: "add".to_string(),
            }
        })
        .collect();

    let files_to_modify: Vec<SyncFileEntry> = diff
        .files_to_modify
        .iter()
        .map(|path| {
            let entry = &official_manifest.files[path];
            SyncFileEntry {
                path: path.clone(),
                sha256: entry.sha256.clone(),
                size: entry.size,
                change_type: "modify".to_string(),
            }
        })
        .collect();

    let total_size: u64 = files_to_add
        .iter()
        .map(|f| f.size)
        .chain(files_to_modify.iter().map(|f| f.size))
        .sum();

    info!(
        "资源同步检查: v{} -> v{} (新增 {} / 修改 {})",
        local_manifest.data_version,
        official_manifest.data_version,
        files_to_add.len(),
        files_to_modify.len()
    );

    Ok(ResourceSyncInfo {
        available: !files_to_add.is_empty() || !files_to_modify.is_empty(),
        new_version: official_manifest.data_version,
        current_version: local_manifest.data_version,
        files_to_add,
        files_to_modify,
        total_size,
    })
}

/// 应用选中的文件同步。
///
/// 将指定文件从 `data/.official/game_data/` 复制到 `data/game_data/`，
/// 更新 `data/data_manifest.json`，完成后删除 `data/.official/`。
#[tauri::command]
pub async fn apply_resource_sync(
    _app: AppHandle,
    state: State<'_, ResourceSyncState>,
    selected_files: Vec<String>,
) -> Result<ResourceSyncResult, String> {
    // 防止并发
    {
        let mut locked = state
            .syncing
            .lock()
            .map_err(|e| format!("锁失败: {e}"))?;
        if *locked {
            return Err("已有同步进行中".to_string());
        }
        *locked = true;
    }

    let result = sync::apply_selected_files(&get_data_dir(), &selected_files).map_err(|e| {
        // 出错时也要解锁
        let mut locked = state.syncing.lock().unwrap();
        *locked = false;
        e.to_string()
    });

    // 解锁
    {
        let mut locked = state.syncing.lock().map_err(|_| "锁错误".to_string())?;
        *locked = false;
    }

    result
}

// ─── 辅助 ────────────────────────────────────────────────────

/// 获取本地数据版本号（供前端显示）。
#[tauri::command]
pub fn get_data_version() -> Result<u64, String> {
    let data_dir = get_data_dir().clone();
    let manifest_path = data_dir.join("data_manifest.json");
    if manifest_path.exists() {
        let manifest =
            DataManifest::load(&manifest_path).map_err(|e| format!("读取清单失败: {e}"))?;
        Ok(manifest.data_version)
    } else {
        Ok(0)
    }
}
