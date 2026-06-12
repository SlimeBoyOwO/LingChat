//! 数据资源自动更新模块。
//!
//! 设计思路:
//! - 默认游戏资源（characters, backgrounds, scripts 等）通过 `data_manifest.json`
//!   清单进行版本管理。
//! - 清单中列出的文件是"默认资源"，不在清单中的文件视为"用户数据"——更新时绝不触碰。
//! - 更新流程: 比较清单版本 → 下载 data_files.zip → 解压到临时目录 → 选择性合并。
//!
//! 前端通过两个 Tauri 命令使用此模块:
//! - `check_data_update`: 检查是否有可用数据更新
//! - `apply_data_update`: 执行数据更新

pub mod manifest;
pub mod sync;

use std::sync::Mutex;

use tauri::{AppHandle, Emitter, State};
use tracing::{error, info};

use crate::api::data_dir;

use self::manifest::{DataUpdateInfo, DataUpdateResult};
use self::sync::{compare_manifests, fetch_remote_manifest, load_local_manifest, perform_data_update, save_local_manifest};

// ─── 更新状态 ────────────────────────────────────────────────

/// 数据更新全局状态（防止并发更新）。
pub struct DataUpdateState {
    /// 是否正在执行更新
    pub updating: Mutex<bool>,
}

impl Default for DataUpdateState {
    fn default() -> Self {
        Self {
            updating: Mutex::new(false),
        }
    }
}

// ─── Tauri 命令 ──────────────────────────────────────────────

/// 检查是否有可用的数据更新。
///
/// 返回 `DataUpdateInfo`，前端可据此展示更新详情。
#[tauri::command]
pub async fn check_data_update() -> Result<DataUpdateInfo, String> {
    info!("检查数据更新...");

    let data_dir = data_dir();
    let local = load_local_manifest(&data_dir);

    let remote = fetch_remote_manifest().await?;

    match local {
        None => {
            // 首次运行，没有本地清单 — 写入远程清单但不执行更新
            // （因为初始安装已包含最新数据）
            info!("未找到本地清单，使用远程清单作为基线");
            if let Err(e) = save_local_manifest(&data_dir, &remote) {
                error!("保存基线清单失败: {e}");
            }
            Ok(DataUpdateInfo {
                available: false,
                new_version: remote.data_version,
                current_version: remote.data_version,
                files_to_add: vec![],
                files_to_modify: vec![],
                files_to_remove: vec![],
                total_download_size: 0,
            })
        }
        Some(local) => {
            if remote.data_version <= local.data_version {
                info!(
                    "数据已是最新 (local={}, remote={})",
                    local.data_version, remote.data_version
                );
                Ok(DataUpdateInfo {
                    available: false,
                    new_version: remote.data_version,
                    current_version: local.data_version,
                    files_to_add: vec![],
                    files_to_modify: vec![],
                    files_to_remove: vec![],
                    total_download_size: 0,
                })
            } else {
                let info = compare_manifests(&local, &remote);
                info!(
                    "发现数据更新: v{} -> v{} (新增 {} / 修改 {} / 移除 {})",
                    local.data_version,
                    remote.data_version,
                    info.files_to_add.len(),
                    info.files_to_modify.len(),
                    info.files_to_remove.len(),
                );
                Ok(info)
            }
        }
    }
}

/// 执行数据更新。
///
/// 下载 data_files.zip，解压，合并文件，更新本地清单。
/// 通过 `data-update-progress` 事件向前端报告进度。
#[tauri::command]
pub async fn apply_data_update(
    app: AppHandle,
    state: State<'_, DataUpdateState>,
) -> Result<DataUpdateResult, String> {
    // 防止并发更新
    {
        let mut updating = state.updating.lock().map_err(|e| format!("锁失败: {e}"))?;
        if *updating {
            return Err("数据更新正在进行中".to_string());
        }
        *updating = true;
    }

    let result = do_apply_data_update(&app).await;

    // 解锁
    {
        let mut updating = state.updating.lock().map_err(|e| format!("锁失败: {e}"))?;
        *updating = false;
    }

    // 通知前端更新结果
    match &result {
        Ok(r) => {
            let _ = app.emit("data-update-complete", serde_json::json!({
                "success": true,
                "message": r.message,
                "newVersion": r.new_version,
            }));
        }
        Err(e) => {
            let _ = app.emit("data-update-complete", serde_json::json!({
                "success": false,
                "message": e,
            }));
        }
    }

    result
}

async fn do_apply_data_update(app: &AppHandle) -> Result<DataUpdateResult, String> {
    let data_dir = data_dir();

    let local = load_local_manifest(&data_dir)
        .ok_or_else(|| "未找到本地清单，无法执行数据更新".to_string())?;

    let remote = fetch_remote_manifest().await?;

    if remote.data_version <= local.data_version {
        return Err("数据已是最新版本".to_string());
    }

    perform_data_update(&data_dir, &local, &remote, app).await
}
