//! 同步引擎 — Push/Pull 编排逻辑。
//!
//! 负责：
//! - 扫描本地 data/ 目录生成 CompleteManifest
//! - 与对端清单对比生成 SyncPlan
//! - 按计划执行文件传输（推送/拉取）
//! - 发送进度事件并检查取消标志
//! - 锁定文件暂存回退（.lan_sync_staging/）

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Instant;

use tauri::{AppHandle, Emitter};
use tracing::{error, info, warn};

use crate::api::data_dir;

use super::client;
use super::db_sync;
use super::manifest as sync_manifest;
use super::messages::{
    DeviceIdentity, PeerInfo, SyncDirection, SyncPlan, SyncProgressEvent, SyncResult,
};
use super::staging;

/// 生成推送计划（本地 → 对端）。
pub async fn plan_push(identity: &DeviceIdentity, peer: &PeerInfo) -> Result<SyncPlan, String> {
    let data_dir = data_dir();

    let local_manifest =
        sync_manifest::build_complete_manifest(&data_dir, None, &identity.device_id)
            .map_err(|e| format!("扫描本地文件失败: {e}"))?;

    let remote_manifest = client::fetch_remote_manifest(peer).await?;

    let (files_to_transfer, files_to_delete) =
        sync_manifest::diff_manifests(&remote_manifest, &local_manifest);

    let total_bytes: u64 = files_to_transfer.iter().map(|f| f.size).sum();

    Ok(SyncPlan {
        direction: SyncDirection::Push,
        peer: peer.clone(),
        files_to_transfer,
        files_to_delete,
        total_bytes,
    })
}

/// 生成拉取计划（对端 → 本地）。
pub async fn plan_pull(identity: &DeviceIdentity, peer: &PeerInfo) -> Result<SyncPlan, String> {
    let data_dir = data_dir();

    let remote_manifest = client::fetch_remote_manifest(peer).await?;

    let local_manifest =
        sync_manifest::build_complete_manifest(&data_dir, None, &identity.device_id)
            .map_err(|e| format!("扫描本地文件失败: {e}"))?;

    let (files_to_transfer, files_to_delete) =
        sync_manifest::diff_manifests(&local_manifest, &remote_manifest);

    let total_bytes: u64 = files_to_transfer.iter().map(|f| f.size).sum();

    Ok(SyncPlan {
        direction: SyncDirection::Pull,
        peer: peer.clone(),
        files_to_transfer,
        files_to_delete,
        total_bytes,
    })
}

/// 执行推送计划（本地 → 对端）。
pub async fn execute_push(
    plan: &SyncPlan,
    app: &AppHandle,
    cancel: &AtomicBool,
) -> Result<SyncResult, String> {
    let data_dir = data_dir();
    let total = plan.files_to_transfer.len() as u64 + plan.files_to_delete.len() as u64;
    let mut current: u64 = 0;
    let mut bytes_transferred: u64 = 0;
    let mut failed_count: u64 = 0;
    let mut files_ok: u64 = 0;
    let mut deletes_ok: u64 = 0;
    let start = Instant::now();

    emit_progress(
        app,
        "transferring",
        current,
        total,
        0,
        None,
        0,
        Some("开始推送...".to_string()),
    );

    // 快速健康检查：确保对端可达
    if let Err(e) = client::check_peer_health(&plan.peer).await {
        return Ok(SyncResult {
            success: false,
            direction: "push".to_string(),
            files_downloaded: 0,
            files_deleted: 0,
            files_staged: 0,
            bytes_transferred: 0,
            message: format!("对端不可达: {e}"),
        });
    }

    for op in &plan.files_to_transfer {
        if cancel.load(Ordering::SeqCst) {
            return Ok(cancelled_result(
                "push", files_ok, deletes_ok, 0, bytes_transferred,
            ));
        }

        let local_path = data_dir.join(&op.path);

        emit_progress(
            app,
            "transferring",
            current,
            total,
            if total > 0 {
                ((current * 100) / total) as u32
            } else {
                0
            },
            Some(op.path.clone()),
            bytes_transferred,
            Some(format!("正在推送: {}", op.path)),
        );

        match client::upload_file(&plan.peer, &local_path, &op.path).await {
            Ok(()) => {
                bytes_transferred += op.size;
                files_ok += 1;
                current += 1;
            }
            Err(e) => {
                error!("推送文件失败 [{}]: {}", op.path, e);
                failed_count += 1;
                current += 1;
            }
        }
    }

    for path in &plan.files_to_delete {
        if cancel.load(Ordering::SeqCst) {
            return Ok(cancelled_result(
                "push", files_ok, deletes_ok, 0, bytes_transferred,
            ));
        }

        match client::push_delete(&plan.peer, path).await {
            Ok(()) => {
                info!("已通知对端删除: {}", path);
                deletes_ok += 1;
                current += 1;
            }
            Err(e) => {
                warn!("通知对端删除失败 [{}]: {}", path, e);
                failed_count += 1;
                current += 1;
            }
        }
    }

    // ─── 数据库记录同步 ────────────────────────────────────
    let mut db_synced = false;
    if failed_count == 0 {
        emit_progress(
            app,
            "syncing-database",
            current,
            total,
            95,
            None,
            bytes_transferred,
            Some("正在同步数据库记录...".to_string()),
        );
        match db_sync::export_all_records(&data_dir).await {
            Ok(records) => match client::push_db_records(&plan.peer, &records).await {
                Ok(()) => {
                    db_synced = true;
                    info!("数据库记录推送完成");
                }
                Err(e) => {
                    warn!("推送数据库记录失败（非致命）: {e}");
                }
            },
            Err(e) => {
                warn!("导出数据库记录失败（非致命）: {e}");
            }
        }
    }

    let elapsed = start.elapsed();
    let success = failed_count == 0;
    let mut message = build_message(success, "push", files_ok, plan.files_to_transfer.len(), failed_count, bytes_transferred, elapsed.as_secs_f64());
    if db_synced {
        message.push_str("（含数据库同步）");
    }

    emit_progress(app, "complete", current, total, 100, None, bytes_transferred, Some(message.clone()));

    Ok(SyncResult {
        success,
        direction: "push".to_string(),
        files_downloaded: files_ok,
        files_deleted: deletes_ok,
        files_staged: 0,
        bytes_transferred,
        message,
    })
}

/// 执行拉取计划（对端 → 本地）。
pub async fn execute_pull(
    plan: &SyncPlan,
    app: &AppHandle,
    cancel: &AtomicBool,
) -> Result<SyncResult, String> {
    let data_dir = data_dir();
    let total = plan.files_to_transfer.len() as u64 + plan.files_to_delete.len() as u64;
    let mut current: u64 = 0;
    let mut bytes_transferred: u64 = 0;
    let mut failed_count: u64 = 0;
    let mut files_ok: u64 = 0;
    let mut files_staged: u64 = 0;
    let mut deletes_ok: u64 = 0;
    let start = Instant::now();

    emit_progress(
        app,
        "transferring",
        current,
        total,
        0,
        None,
        0,
        Some("开始拉取...".to_string()),
    );

    // 快速健康检查：确保对端可达
    if let Err(e) = client::check_peer_health(&plan.peer).await {
        return Ok(SyncResult {
            success: false,
            direction: "pull".to_string(),
            files_downloaded: 0,
            files_deleted: 0,
            files_staged: 0,
            bytes_transferred: 0,
            message: format!("对端不可达: {e}"),
        });
    }

    for op in &plan.files_to_transfer {
        if cancel.load(Ordering::SeqCst) {
            return Ok(cancelled_result(
                "pull", files_ok, deletes_ok, files_staged, bytes_transferred,
            ));
        }

        let dest_path = data_dir.join(&op.path);

        emit_progress(
            app,
            "transferring",
            current,
            total,
            if total > 0 {
                ((current * 100) / total) as u32
            } else {
                0
            },
            Some(op.path.clone()),
            bytes_transferred,
            Some(format!("正在拉取: {}", op.path)),
        );

        match client::download_file(&plan.peer, &op.path, &dest_path).await {
            Ok(()) => {
                bytes_transferred += op.size;
                files_ok += 1;
                current += 1;
            }
            Err(e) => {
                // 尝试回退：如果文件已下载到 .tmp 但重命名失败（被锁定），则暂存
                let tmp = tmp_path_for(&dest_path);
                if tmp.exists() {
                    match staging::stage_file(&data_dir, &op.path, &tmp) {
                        Ok(()) => {
                            info!("文件已暂存（重启后生效）: {}", op.path);
                            files_staged += 1;
                            let _ = std::fs::remove_file(&tmp);
                            current += 1;
                            continue;
                        }
                        Err(se) => {
                            error!("暂存文件失败 [{}]: {}", op.path, se);
                        }
                    }
                }
                error!("下载文件失败 [{}]: {}", op.path, e);
                failed_count += 1;
                current += 1;
            }
        }
    }

    for path in &plan.files_to_delete {
        if cancel.load(Ordering::SeqCst) {
            return Ok(cancelled_result(
                "pull", files_ok, deletes_ok, files_staged, bytes_transferred,
            ));
        }

        let local_path = data_dir.join(path);
        if local_path.exists() {
            let trash_dir = data_dir.join(".trash");
            let _ = std::fs::create_dir_all(&trash_dir);
            let file_name = local_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let trash_path = trash_dir.join(&file_name);
            if let Err(e) = std::fs::rename(&local_path, &trash_path) {
                warn!("本地删除失败 [{}]: {}", path, e);
                failed_count += 1;
            } else {
                info!("已删除本地文件: {}", path);
                deletes_ok += 1;
            }
        }
        current += 1;
    }

    // ─── 数据库记录同步 ────────────────────────────────────
    let mut db_staged = false;
    if failed_count == 0 {
        emit_progress(
            app,
            "syncing-database",
            current,
            total,
            95,
            None,
            bytes_transferred,
            Some("正在拉取数据库记录...".to_string()),
        );
        match client::fetch_db_records(&plan.peer).await {
            Ok(records) => match db_sync::stage_db_records(&data_dir, &records) {
                Ok(()) => {
                    db_staged = true;
                    info!("数据库记录已暂存，将在下次启动时导入");
                }
                Err(e) => {
                    warn!("暂存数据库记录失败（非致命）: {e}");
                }
            },
            Err(e) => {
                warn!("拉取数据库记录失败（非致命）: {e}");
            }
        }
    }

    let elapsed = start.elapsed();
    let success = failed_count == 0;
    let mut message = build_message(success, "pull", files_ok, plan.files_to_transfer.len(), failed_count, bytes_transferred, elapsed.as_secs_f64());
    if files_staged > 0 {
        message.push_str(&format!(
            "；{} 个文件因被占用已暂存，重启后自动生效",
            files_staged
        ));
    }
    if db_staged {
        message.push_str("；数据库记录已暂存，重启后自动导入");
    }

    emit_progress(
        app,
        "complete",
        current,
        total,
        100,
        None,
        bytes_transferred,
        Some(message.clone()),
    );

    Ok(SyncResult {
        success,
        direction: "pull".to_string(),
        files_downloaded: files_ok,
        files_deleted: deletes_ok,
        files_staged,
        bytes_transferred,
        message,
    })
}

// ─── 辅助 ────────────────────────────────────────────────────

fn emit_progress(
    app: &AppHandle,
    phase: &str,
    current: u64,
    total: u64,
    progress: u32,
    current_file: Option<String>,
    bytes_transferred: u64,
    message: Option<String>,
) {
    let event = SyncProgressEvent {
        phase: phase.to_string(),
        current,
        total,
        progress,
        current_file,
        bytes_transferred,
        message,
    };
    let _ = app.emit("lan-sync-progress", &event);
}

fn cancelled_result(
    direction: &str,
    files_ok: u64,
    deletes_ok: u64,
    files_staged: u64,
    bytes_transferred: u64,
) -> SyncResult {
    SyncResult {
        success: false,
        direction: direction.to_string(),
        files_downloaded: files_ok,
        files_deleted: deletes_ok,
        files_staged,
        bytes_transferred,
        message: "用户取消同步".to_string(),
    }
}

fn build_message(
    success: bool,
    direction: &str,
    files_ok: u64,
    total_files: usize,
    failed_count: u64,
    bytes_transferred: u64,
    elapsed_secs: f64,
) -> String {
    let verb = if direction == "pull" { "拉取" } else { "推送" };
    if success {
        format!(
            "{}完成：{} 个文件，{} 字节，耗时 {:.1}s",
            verb, files_ok, bytes_transferred, elapsed_secs
        )
    } else {
        format!(
            "{}部分完成：{}/{} 个文件成功，{} 个失败",
            verb,
            files_ok,
            total_files,
            failed_count
        )
    }
}

/// 推算下载时的临时文件路径（与 `client::download_file` 内的命名规则一致）。
fn tmp_path_for(dest: &Path) -> PathBuf {
    let ext = dest
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    dest.with_extension(format!("{}.tmp", ext))
}
