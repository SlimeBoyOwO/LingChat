//! 数据更新同步逻辑 — 下载、校验、合并。

use std::collections::HashSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tauri::Emitter;
use tracing::{info, warn};

use super::manifest::{DataManifest, DataUpdateInfo, DataUpdateResult};

// ─── 常量 ────────────────────────────────────────────────────

/// GitHub Release 的基础下载 URL
const RELEASE_DOWNLOAD_BASE: &str =
    "https://github.com/SlimeBoyOwO/LingChat/releases/latest/download";
/// 本地清单文件名
const MANIFEST_FILENAME: &str = "data_manifest.json";
/// 数据 zip 文件名
const DATA_ZIP_FILENAME: &str = "data_files.zip";
/// 临时解压目录名
const TEMP_EXTRACT_DIR: &str = ".data_update_tmp";
/// 软删除目录名（被移除的默认文件移到这里）
const TRASH_DIR: &str = ".trash";

// ─── 清单读写 ────────────────────────────────────────────────

/// 读取本地清单（如果存在）。
pub fn load_local_manifest(data_dir: &Path) -> Option<DataManifest> {
    let path = data_dir.join(MANIFEST_FILENAME);
    if !path.exists() {
        return None;
    }
    match fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<DataManifest>(&json) {
            Ok(m) => Some(m),
            Err(e) => {
                warn!("本地清单格式损坏: {e}");
                None
            }
        },
        Err(e) => {
            warn!("无法读取本地清单: {e}");
            None
        }
    }
}

/// 将清单写入本地 data 目录。
pub fn save_local_manifest(data_dir: &Path, manifest: &DataManifest) -> io::Result<()> {
    let path = data_dir.join(MANIFEST_FILENAME);
    let tmp = data_dir.join(format!(".{MANIFEST_FILENAME}.tmp"));
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(&tmp, &json)?;
    fs::rename(&tmp, &path)?;
    info!("本地清单已更新 -> data_version={}", manifest.data_version);
    Ok(())
}

// ─── 远程清单获取 ────────────────────────────────────────────

/// 从 GitHub Release 获取远程清单。
pub async fn fetch_remote_manifest() -> Result<DataManifest, String> {
    let url = format!("{RELEASE_DOWNLOAD_BASE}/{MANIFEST_FILENAME}");
    info!("正在获取远程清单: {url}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("LingChat-Updater/2.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("获取远程清单失败: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("服务器返回 {}", resp.status()));
    }

    let json = resp
        .text()
        .await
        .map_err(|e| format!("读取远程清单响应失败: {e}"))?;

    serde_json::from_str::<DataManifest>(&json)
        .map_err(|e| format!("解析远程清单 JSON 失败: {e}"))
}

// ─── 清单比较 ────────────────────────────────────────────────

/// 比较两个清单，生成更新信息。
pub fn compare_manifests(old: &DataManifest, new: &DataManifest) -> DataUpdateInfo {
    let old_keys: HashSet<&String> = old.files.keys().collect();
    let new_keys: HashSet<&String> = new.files.keys().collect();

    let mut files_to_add: Vec<String> = new_keys
        .difference(&old_keys)
        .map(|&k| k.clone())
        .collect();

    let mut files_to_modify: Vec<String> = old_keys
        .intersection(&new_keys)
        .filter(|&&k| {
            old.files[k].sha256 != new.files[k].sha256
                || old.files[k].size != new.files[k].size
        })
        .map(|&k| k.clone())
        .collect();

    let files_to_remove: Vec<String> = old_keys
        .difference(&new_keys)
        .map(|&k| k.clone())
        .collect();

    files_to_add.sort();
    files_to_modify.sort();

    let total_changes =
        files_to_add.len() + files_to_modify.len() + files_to_remove.len();

    DataUpdateInfo {
        available: total_changes > 0,
        new_version: new.data_version,
        current_version: old.data_version,
        files_to_add,
        files_to_modify,
        files_to_remove,
        total_download_size: 0, // 将在检查阶段补充
    }
}

// ─── 下载与解压 ──────────────────────────────────────────────

/// 计算文件的 SHA-256 哈希（hex 字符串）
fn compute_sha256(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// 下载数据 zip 包到临时路径，返回临时文件路径。
async fn download_data_zip(temp_dir: &Path) -> Result<PathBuf, String> {
    let url = format!("{RELEASE_DOWNLOAD_BASE}/{DATA_ZIP_FILENAME}");
    info!("正在下载数据包: {url}");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3600)) // 大文件长超时
        .user_agent("LingChat-Updater/2.0")
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {e}"))?;

    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("下载数据包失败: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("服务器返回 {}", resp.status()));
    }

    let total_size = resp.content_length();
    let zip_path = temp_dir.join(DATA_ZIP_FILENAME);

    let mut file = File::create(&zip_path)
        .map_err(|e| format!("创建临时文件失败: {e}"))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("下载数据块失败: {e}"))?;
        io::Write::write_all(&mut file, &chunk)
            .map_err(|e| format!("写入数据块失败: {e}"))?;
        downloaded += chunk.len() as u64;
    }

    // 校验大小（如果服务器提供了 Content-Length）
    if let Some(expected) = total_size {
        if downloaded != expected {
            return Err(format!(
                "下载大小不匹配: 期望 {expected} 字节，实际 {downloaded} 字节"
            ));
        }
    }

    info!("数据包下载完成: {} 字节", downloaded);
    Ok(zip_path)
}

/// 解压 zip 到目标目录。
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), String> {
    info!("正在解压数据包到临时目录...");

    let file = File::open(zip_path).map_err(|e| format!("打开 zip 文件失败: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("读取 zip 文件失败: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 zip 条目 {i} 失败: {e}"))?;

        let entry_path = entry
            .enclosed_name()
            .ok_or_else(|| format!("zip 条目 {i} 路径不安全"))?;

        let out_path = dest_dir.join(entry_path);

        if entry.is_dir() {
            fs::create_dir_all(&out_path)
                .map_err(|e| format!("创建目录 {} 失败: {e}", out_path.display()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("创建父目录 {} 失败: {e}", parent.display()))?;
            }
            let mut out_file = File::create(&out_path)
                .map_err(|e| format!("创建文件 {} 失败: {e}", out_path.display()))?;
            io::copy(&mut entry, &mut out_file)
                .map_err(|e| format!("解压文件 {} 失败: {e}", out_path.display()))?;
        }
    }

    info!("解压完成");
    Ok(())
}

// ─── 应用更新 ────────────────────────────────────────────────

/// 将解压后的文件合并到 data 目录，删除已移除的文件，最后写入新清单。
pub fn apply_extracted_files(
    data_dir: &Path,
    extract_dir: &Path,
    new_manifest: &DataManifest,
    old_manifest: &DataManifest,
    app_handle: &tauri::AppHandle,
) -> Result<DataUpdateResult, String> {
    let trash_dir = data_dir.join(TRASH_DIR);
    let old_keys: HashSet<&String> = old_manifest.files.keys().collect();
    let new_keys: HashSet<&String> = new_manifest.files.keys().collect();

    // ── 1. 删除已移除的文件（移动到 .trash/） ──
    let removed: Vec<&String> = old_keys.difference(&new_keys).copied().collect();
    for rel_path in &removed {
        let target = data_dir.join(rel_path);
        if target.exists() {
            let trash_path = trash_dir.join(rel_path);
            if let Some(parent) = trash_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::rename(&target, &trash_path) {
                warn!("移动文件到回收站失败 {}: {e}", rel_path);
            } else {
                info!("已移除: {rel_path}");
            }
        }
    }

    // ── 2. 复制新增/修改的文件 ──
    let to_update: Vec<&String> = new_keys
        .iter()
        .filter(|&&k| {
            // 新增或修改
            !old_keys.contains(k)
                || old_manifest.files[k].sha256 != new_manifest.files[k].sha256
        })
        .copied()
        .collect();

    let total = to_update.len();
    for (idx, rel_path) in to_update.iter().enumerate() {
        let src = extract_dir.join(rel_path);
        let dst = data_dir.join(rel_path);

        if !src.exists() {
            warn!("解压目录中缺少文件: {rel_path}，跳过");
            continue;
        }

        // 原子写入：先写 .tmp，再重命名
        let tmp = data_dir.join(format!(".{}.tmp", rel_path.replace('/', "_")));
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("创建目录失败: {e}"))?;
        }

        fs::copy(&src, &tmp).map_err(|e| format!("复制文件失败 {rel_path}: {e}"))?;
        fs::rename(&tmp, &dst).map_err(|e| format!("重命名文件失败 {rel_path}: {e}"))?;

        // 发送进度事件给前端
        let progress = ((idx + 1) as f64 / total as f64 * 100.0) as u32;
        let _ = app_handle.emit("data-update-progress", serde_json::json!({
            "current": idx + 1,
            "total": total,
            "progress": progress,
            "currentFile": rel_path,
        }));
    }

    // ── 3. 写入新清单 ──
    save_local_manifest(data_dir, new_manifest)
        .map_err(|e| format!("写入新清单失败: {e}"))?;

    Ok(DataUpdateResult {
        success: true,
        message: format!(
            "数据更新完成: 新增 {} 个文件, 修改 {} 个文件, 移除 {} 个文件",
            new_keys.difference(&old_keys).count(),
            to_update.len() - new_keys.difference(&old_keys).count(),
            removed.len(),
        ),
        new_version: new_manifest.data_version,
    })
}

// ─── 完整更新流程 ────────────────────────────────────────────

/// 执行完整的数据更新流程。
///
/// 1. 比较本地与远程清单
/// 2. 如有更新，下载 data_files.zip
/// 3. 解压到临时目录
/// 4. 合并文件
/// 5. 清理临时文件
pub async fn perform_data_update(
    data_dir: &Path,
    old_manifest: &DataManifest,
    new_manifest: &DataManifest,
    app_handle: &tauri::AppHandle,
) -> Result<DataUpdateResult, String> {
    // 创建临时目录
    let temp_dir = data_dir.join(TEMP_EXTRACT_DIR);
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    fs::create_dir_all(&temp_dir).map_err(|e| format!("创建临时目录失败: {e}"))?;

    // 通知前端：开始下载
    let _ = app_handle.emit("data-update-progress", serde_json::json!({
        "phase": "downloading",
        "progress": 0,
        "message": "正在下载数据包...",
    }));

    // 下载 zip
    let zip_path = download_data_zip(&temp_dir).await?;

    // 通知前端：开始解压
    let _ = app_handle.emit("data-update-progress", serde_json::json!({
        "phase": "extracting",
        "progress": 0,
        "message": "正在解压数据包...",
    }));

    // 解压
    let extract_dir = temp_dir.join("data");
    extract_zip(&zip_path, &extract_dir)?;

    // 通知前端：开始合并
    let _ = app_handle.emit("data-update-progress", serde_json::json!({
        "phase": "applying",
        "progress": 0,
        "message": "正在合并文件...",
    }));

    // 应用文件变更
    let result = apply_extracted_files(data_dir, &extract_dir, new_manifest, old_manifest, app_handle)?;

    // 清理临时文件
    let _ = fs::remove_dir_all(&temp_dir);

    info!("数据更新完成: {}", result.message);
    Ok(result)
}
