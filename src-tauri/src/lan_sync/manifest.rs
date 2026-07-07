//! LAN 同步专用清单工具。
//!
//! 扩展自 `data_update::manifest`，额外处理不在标准清单中的运行时文件。

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};
use tracing::warn;

use crate::manifest::{DataManifest, FileEntry};

use super::messages::CompleteManifest;

/// 扫描整个 data/ 目录（排除 third_party/），构建 CompleteManifest。
///
/// - 已在 `data_manifest.json` 中的文件复用清单中的 SHA-256
/// - 不在清单中的运行时文件实时计算 SHA-256
/// - `modified_at` 优先取清单中的值（如果有），否则取文件系统时间戳
pub fn build_complete_manifest(
    data_dir: &Path,
    manifest: Option<&DataManifest>,
    device_id: &str,
) -> io::Result<CompleteManifest> {
    let mut files: HashMap<String, FileEntry> = HashMap::new();
    let mut runtime_files: HashMap<String, FileEntry> = HashMap::new();

    let manifest_files = manifest.map(|m| &m.files);
    let data_version = manifest.map_or(0, |m| m.data_version);

    scan_dir(data_dir, data_dir, manifest_files, &mut files, &mut runtime_files)?;

    Ok(CompleteManifest {
        device_id: device_id.to_string(),
        data_version,
        files,
        runtime_files,
    })
}

/// 递归扫描目录，收集所有文件信息。
fn scan_dir(
    base: &Path,
    current: &Path,
    manifest_files: Option<&HashMap<String, FileEntry>>,
    files: &mut HashMap<String, FileEntry>,
    runtime_files: &mut HashMap<String, FileEntry>,
) -> io::Result<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        // 跳过隐藏文件、临时文件和 SQLite 日志文件（这些是瞬态的，可能在传输间消失）
        if file_name.starts_with('.') || file_name.ends_with(".tmp") {
            continue;
        }
        if file_name.ends_with("-wal")
            || file_name.ends_with("-shm")
            || file_name.ends_with("-journal")
            || file_name.ends_with(".db")
        {
            continue;
        }

        if path.is_dir() {
            // 排除 third_party/ 整个目录
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if rel == "third_party" || rel.starts_with("third_party/") {
                continue;
            }
            // 跳过回收站
            if rel == ".trash" || rel.starts_with(".trash/") {
                continue;
            }
            scan_dir(base, &path, manifest_files, files, runtime_files)?;
        } else if path.is_file() {
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");

            let modified_at = path
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            let size = path.metadata().map(|m| m.len()).unwrap_or(0);

            if let Some(mf) = manifest_files {
                if let Some(entry) = mf.get(&rel) {
                    // 在清单中 → 放入 files，复用已知哈希
                    let mut fe = entry.clone();
                    // 如果清单中没有 modified_at，补充文件系统时间戳
                    if fe.modified_at == 0 {
                        fe.modified_at = modified_at;
                    }
                    files.insert(rel, fe);
                    continue;
                }
            }

            // 不在清单中 → 运行时文件，实时计算 SHA-256
            let sha256 = match compute_sha256(&path) {
                Ok(h) => h,
                Err(e) => {
                    warn!("无法计算文件哈希 {:?}: {}", path, e);
                    continue; // 跳过无法读取的文件
                }
            };

            runtime_files.insert(
                rel,
                FileEntry {
                    sha256,
                    size,
                    modified_at,
                },
            );
        }
    }

    Ok(())
}

/// 计算文件的 SHA-256 哈希（hex 字符串）。
fn compute_sha256(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// 比较两个 CompleteManifest，生成同步计划。
///
/// - `files_to_transfer`: 对端有而本地没有、或对端版本更新的文件
/// - `files_to_delete`: 本地有而对端没有的文件
/// - 冲突按照 `modified_at` 判断，时间戳更新者获胜
pub fn diff_manifests(
    local: &CompleteManifest,
    remote: &CompleteManifest,
) -> (Vec<super::messages::SyncFileOp>, Vec<String>) {
    let mut to_transfer = Vec::new();
    let mut to_delete = Vec::new();

    // 合并 files 和 runtime_files 为统一视图
    let local_all: HashMap<&str, &FileEntry> = local
        .files
        .iter()
        .chain(local.runtime_files.iter())
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    let remote_all: HashMap<&str, &FileEntry> = remote
        .files
        .iter()
        .chain(remote.runtime_files.iter())
        .map(|(k, v)| (k.as_str(), v))
        .collect();

    // 找出远端有而本地没有、或远端更新的文件
    for (path, remote_entry) in &remote_all {
        match local_all.get(path) {
            None => {
                to_transfer.push(super::messages::SyncFileOp {
                    path: path.to_string(),
                    sha256: remote_entry.sha256.clone(),
                    size: remote_entry.size,
                    reason: "new".to_string(),
                });
            }
            Some(local_entry) => {
                if local_entry.sha256 != remote_entry.sha256 {
                    // SHA-256 不同 → 比较时间戳
                    if remote_entry.modified_at > local_entry.modified_at {
                        to_transfer.push(super::messages::SyncFileOp {
                            path: path.to_string(),
                            sha256: remote_entry.sha256.clone(),
                            size: remote_entry.size,
                            reason: "newer".to_string(),
                        });
                    } else if remote_entry.modified_at < local_entry.modified_at {
                        // 本地更新，不传输
                    } else {
                        // 时间戳相同（罕见），跳过
                        warn!(
                            "冲突文件 {} 时间戳相同，跳过同步 (local={}, remote={})",
                            path, local_entry.sha256, remote_entry.sha256
                        );
                    }
                }
                // SHA-256 相同 → 跳过
            }
        }
    }

    // 找出本地有而远端没有的文件
    for path in local_all.keys() {
        if !remote_all.contains_key(path) {
            to_delete.push(path.to_string());
        }
    }

    (to_transfer, to_delete)
}
