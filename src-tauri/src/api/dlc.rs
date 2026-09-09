//! DLC（即插即用的剧本包）管理命令。
//!
//! DLC 就是一个 zip 格式的剧本包：内含一个带 `story_config.yaml` 的剧本目录
//! （目录可以在 zip 根，也可以包一层同名文件夹）。导入 = 在扫描区外有界解压、
//! 校验后原子提交到 `data/game_data/scripts/standalone/` 并立刻注册；卸载 = 从
//! ScriptManager 摘除并用可恢复隔离事务删除。只有带 `dlc.json` 标记的目录能被
//! 卸载——内置剧本目录没有这个标记，不会被误删。

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::ai_service::game_system::script_engine::ScriptManager;
use crate::utils::script_paths::sanitize_folder_name;
use crate::AppState;

const MAX_ZIP_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ZIP_ENTRIES: usize = 8_192;
const MAX_ENTRY_NAME_BYTES: usize = 512;
const MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_COMPRESSION_RATIO: u64 = 200;

/// DLC 清单文件（`dlc.json`），随剧本包分发；导入时缺省会补写一份。
/// 全字段保留序列化：补写 imported_at 时不能丢掉作者自带的 name/min_engine 等元数据。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DlcManifest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(default)]
    version: String,
    #[serde(default)]
    author: String,
    /// 需要的最低游戏版本；ScriptManager 在导入、启动扫描和动态加载时强制校验。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    min_engine: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    homepage: Option<String>,
    #[serde(default)]
    imported_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DlcInfo {
    /// 目录名（standalone/<folder_key>）
    pub folder_key: String,
    /// story_config 的 script_name
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_warning: Option<String>,
    pub version: String,
    pub author: String,
    pub imported_at: String,
}

/// Reserves the shared script lifecycle flag while DLC files/catalog state are
/// being mutated. `start_script` uses compare_exchange on the same flag, so no
/// script can be cloned between an IPC pre-check and package quarantine.
struct ScriptCollectionReservation(Arc<AtomicBool>);

impl Drop for ScriptCollectionReservation {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn standalone_root() -> PathBuf {
    crate::init::static_copy::get_data_dir()
        .join("game_data")
        .join("scripts")
        .join("standalone")
}

fn failed_import_error(data_dir: &Path, target: &Path, message: String) -> String {
    match crate::ai_service::game_system::script_engine::dlc_transaction::discard_rejected_install(
        data_dir, target,
    ) {
        Ok(()) => message,
        Err(cleanup_error) => format!(
            "{}；失败目录清理也失败，请重启后处理: {:#}",
            message, cleanup_error
        ),
    }
}

fn failed_committed_import_error(
    data_dir: &Path,
    target: &Path,
    transaction_dir: &Path,
    message: String,
) -> String {
    if let Err(error) =
        crate::ai_service::game_system::script_engine::dlc_transaction::discard_rejected_install(
            data_dir, target,
        )
    {
        return format!("{message}；已提交包隔离失败并保留恢复事务: {error:#}");
    }
    match crate::ai_service::game_system::script_engine::dlc_transaction::abort_install_commit(
        data_dir,
        transaction_dir,
    ) {
        Ok(()) => message,
        Err(error) => format!("{message}；取消安装恢复事务失败: {error:#}"),
    }
}

fn read_manifest(dir: &Path) -> DlcManifest {
    fs::read_to_string(dir.join("dlc.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn dlc_info_of(dir: &Path) -> Option<DlcInfo> {
    // 只认带 dlc.json 标记的目录——这是「通过 DLC 包安装」与「内置剧本」的分界线
    if !dir.join("dlc.json").is_file() {
        return None;
    }
    let manifest = read_manifest(dir);
    let folder_key = dir.file_name()?.to_string_lossy().to_string();
    let status = ScriptManager::read_script_config_unchecked(dir).ok();
    Some(DlcInfo {
        folder_key: status
            .as_ref()
            .map(|value| value.folder_key.clone())
            .unwrap_or_else(|| folder_key.clone()),
        name: status
            .as_ref()
            .map(|value| value.name.clone())
            .or_else(|| manifest.name.clone())
            .unwrap_or(folder_key),
        description: status
            .as_ref()
            .map(|value| value.description.clone())
            .or_else(|| manifest.description.clone())
            .unwrap_or_default(),
        content_warning: status.and_then(|value| value.content_warning),
        version: manifest.version,
        author: manifest.author,
        imported_at: manifest.imported_at,
    })
}

#[tauri::command]
pub async fn list_dlcs(app: AppHandle) -> Result<Vec<DlcInfo>, String> {
    let _state = app.state::<AppState>();
    let root = standalone_root();
    let mut out: Vec<DlcInfo> = Vec::new();
    if let Ok(entries) = fs::read_dir(&root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(info) = dlc_info_of(&path) {
                    out.push(info);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[tauri::command]
pub async fn import_dlc(app: AppHandle, zip_path: String) -> Result<DlcInfo, String> {
    let state = app.state::<AppState>();

    // Atomically reserve the same lifecycle flag used by start_script. Merely
    // loading then releasing the service lock leaves a race where a script can
    // start while its package collection is being changed.
    let _collection_reservation = {
        let service = state.ai_service.lock().await;
        let flag = service.script_manager.is_running.clone();
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "剧本正在运行或另一个 DLC 管理操作尚未完成".to_string())?;
        ScriptCollectionReservation(flag)
    };
    let data_dir = state.ai_service.lock().await.data_dir.clone();
    crate::ai_service::game_system::script_engine::dlc_transaction::recover_pending_uninstalls(
        &data_dir,
    );

    let zip_file =
        fs::File::open(&zip_path).map_err(|e| format!("无法打开 DLC 包 '{}': {}", zip_path, e))?;
    let zip_size = zip_file
        .metadata()
        .map_err(|e| format!("读取 DLC 包大小失败: {}", e))?
        .len();
    if zip_size > MAX_ZIP_FILE_BYTES {
        return Err("DLC 压缩包超过 2 GiB 安全上限".to_string());
    }
    let mut archive =
        zip::ZipArchive::new(zip_file).map_err(|e| format!("DLC 包不是有效的 zip: {}", e))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(format!("DLC 条目数超过 {} 个安全上限", MAX_ZIP_ENTRIES));
    }

    // ---- 定位剧本根并预检声明尺寸/压缩比 ----
    let mut names: Vec<String> = Vec::new();
    let mut advertised_total = 0u64;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| format!("读取 DLC 包条目失败: {}", e))?;
        if entry.name_raw().len() > MAX_ENTRY_NAME_BYTES {
            return Err("DLC 条目路径超过 512 字节安全上限".to_string());
        }
        if entry.size() > MAX_ENTRY_BYTES {
            return Err(format!("DLC 条目 '{}' 超过 256 MiB 上限", entry.name()));
        }
        advertised_total = advertised_total
            .checked_add(entry.size())
            .ok_or_else(|| "DLC 解压大小溢出".to_string())?;
        if advertised_total > MAX_TOTAL_UNCOMPRESSED_BYTES {
            return Err("DLC 声明解压总量超过 1 GiB 安全上限".to_string());
        }
        if entry.size() >= 1024 * 1024
            && (entry.compressed_size() == 0
                || entry.size()
                    > entry
                        .compressed_size()
                        .saturating_mul(MAX_COMPRESSION_RATIO))
        {
            return Err(format!("DLC 条目 '{}' 压缩比异常，已拒绝", entry.name()));
        }
        if let Some(mode) = entry.unix_mode() {
            if mode & 0o170000 == 0o120000 {
                return Err("DLC 包不允许符号链接条目".to_string());
            }
        }
        if let Some(enclosed) = entry.enclosed_name() {
            names.push(enclosed.to_string_lossy().replace('\\', "/"));
        } else {
            return Err("DLC 包含可疑路径条目，已拒绝安装".to_string());
        }
    }
    let root_prefix = detect_script_root(&names)?;

    // ---- 目标目录名：带壳包用壳目录名，平铺包用 zip 文件名 ----
    let folder_name = if root_prefix.is_empty() {
        Path::new(&zip_path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default()
    } else {
        root_prefix.trim_end_matches('/').to_string()
    };
    let folder_name =
        sanitize_folder_name(&folder_name).map_err(|e| format!("DLC 目录名非法: {}", e))?;
    if crate::ai_service::game_system::script_engine::dlc_transaction::has_pending_for_folder(
        &data_dir,
        &folder_name,
    )
    .map_err(|error| format!("检查待完成卸载事务失败: {error:#}"))?
    {
        return Err(format!(
            "DLC '{}' 仍有待完成的卸载清理，请重启后再安装",
            folder_name
        ));
    }

    let target = standalone_root().join(&folder_name);
    if target.exists() {
        return Err(format!(
            "已存在同名剧本目录 '{}'，请先卸载旧版",
            folder_name
        ));
    }

    // ---- 在扫描区外解压（剥根；enclosed_name 已做 zip-slip 防护）----
    let staged =
        crate::ai_service::game_system::script_engine::dlc_transaction::begin_install_staging(
            &data_dir,
        )
        .map_err(|error| format!("创建 DLC 安装暂存区失败: {error:#}"))?;
    let extract_result = (|| -> Result<(), String> {
        let mut actual_total = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| format!("读取 DLC 包条目失败: {}", e))?;
            let enclosed = entry.enclosed_name().ok_or("DLC 包含可疑路径条目")?;
            let rel_str = enclosed.to_string_lossy().replace('\\', "/");
            let rel = if root_prefix.is_empty() {
                rel_str.clone()
            } else {
                match rel_str.strip_prefix(&root_prefix) {
                    Some(r) => r.to_string(),
                    None => continue, // 壳外的零散文件不装
                }
            };
            if rel.is_empty() {
                continue;
            }
            let out_path = staged.join(&rel);
            if entry.is_dir() {
                fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut out_file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&out_path)
                    .map_err(|e| format!("创建解压文件失败 '{}': {}", rel, e))?;
                let remaining = MAX_TOTAL_UNCOMPRESSED_BYTES.saturating_sub(actual_total);
                let allowed = MAX_ENTRY_BYTES.min(remaining);
                let copied = std::io::copy(&mut entry.by_ref().take(allowed + 1), &mut out_file)
                    .map_err(|e| e.to_string())?;
                if copied > allowed {
                    return Err(format!("DLC 条目 '{}' 实际解压大小超过安全上限", rel));
                }
                actual_total = actual_total
                    .checked_add(copied)
                    .ok_or_else(|| "DLC 实际解压总量溢出".to_string())?;
                out_file.flush().map_err(|e| e.to_string())?;
                out_file.sync_all().map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    })();

    if let Err(e) = extract_result {
        return Err(failed_import_error(
            &data_dir,
            &staged,
            format!("解压 DLC 包失败: {}", e),
        ));
    }

    // ---- 结构/身份校验：配置有效、版本兼容且显示名尚未被占用 ----
    let staged_status = match ScriptManager::read_script_config(&staged) {
        Ok(status) => status,
        Err(error) => {
            return Err(failed_import_error(
                &data_dir,
                &staged,
                format!("DLC 包缺少有效配置或引擎版本不兼容: {error:#}"),
            ))
        }
    };
    let duplicate_paths = {
        let service = state.ai_service.lock().await;
        service
            .script_manager
            .script_name_claim_paths(&staged_status.name)
    };
    if !duplicate_paths.is_empty() {
        return Err(failed_import_error(
            &data_dir,
            &staged,
            format!(
                "剧本显示名 '{}' 已被现有目录占用或处于重名禁用状态，拒绝覆盖: {:?}",
                staged_status.name, duplicate_paths
            ),
        ));
    }

    // ---- 补写/补全 dlc.json 标记（作者自带的字段全保留，只补 imported_at）----
    let manifest_path = staged.join("dlc.json");
    let mut manifest = read_manifest(&staged);
    if manifest.imported_at.is_empty() {
        manifest.imported_at = chrono::Local::now().to_rfc3339();
    }
    let json = match serde_json::to_string_pretty(&manifest) {
        Ok(json) => json,
        Err(error) => {
            return Err(failed_import_error(
                &data_dir,
                &staged,
                format!("序列化 dlc.json 失败: {}", error),
            ))
        }
    };
    if let Err(error) = crate::ai_service::tools::atomic_replace(&manifest_path, json.as_bytes()) {
        return Err(failed_import_error(
            &data_dir,
            &staged,
            format!("写入 dlc.json 失败: {}", error),
        ));
    }

    // ---- 校验完成后用同盘目录重命名原子提交，再注册进引擎 ----
    let install_transaction =
        match crate::ai_service::game_system::script_engine::dlc_transaction::commit_staged_install(
            &data_dir, &staged, &target,
        ) {
            Ok(transaction) => transaction,
            Err(error) => {
                return Err(failed_import_error(
                    &data_dir,
                    &staged,
                    format!("提交 DLC 安装失败: {error:#}"),
                ))
            }
        };

    // ---- 立刻注册进引擎 ----
    let load_result = {
        let mut service = state.ai_service.lock().await;
        service.script_manager.load_script_dir(&target)
    };
    if let Err(error) = load_result {
        return Err(failed_committed_import_error(
            &data_dir,
            &target,
            &install_transaction,
            format!("DLC 注册失败: {:#}", error),
        ));
    }

    match dlc_info_of(&target) {
        Some(info) => {
            if let Err(error) = crate::ai_service::game_system::script_engine::dlc_transaction::finish_install_commit(
                &data_dir,
                &install_transaction,
            ) {
                // The fully installed package remains usable; durable evidence
                // is intentionally retained for startup to retry parent flushes.
                tracing::warn!("[DLC] 安装已完成，提交事务仍待恢复: {error:#}");
            }
            Ok(info)
        }
        None => {
            let mut service = state.ai_service.lock().await;
            service.script_manager.unload_script_dir(&target);
            drop(service);
            Err(failed_committed_import_error(
                &data_dir,
                &target,
                &install_transaction,
                "DLC 安装后读取信息失败".to_string(),
            ))
        }
    }
}

#[tauri::command]
pub async fn remove_dlc(app: AppHandle, folder_key: String) -> Result<(), String> {
    let state = app.state::<AppState>();

    let _collection_reservation = {
        let service = state.ai_service.lock().await;
        let flag = service.script_manager.is_running.clone();
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| "剧本正在运行或另一个 DLC 管理操作尚未完成".to_string())?;
        ScriptCollectionReservation(flag)
    };

    let folder_name =
        sanitize_folder_name(&folder_key).map_err(|e| format!("DLC 目录名非法: {}", e))?;
    let target = standalone_root().join(&folder_name);
    if !target.is_dir() {
        return Err(format!("DLC 不存在: '{}'", folder_key));
    }
    // 只允许卸载带 dlc.json 标记的目录，内置剧本走不到这里
    if !target.join("dlc.json").is_file() {
        return Err("该剧本不是通过 DLC 包安装的，不能在此卸载".to_string());
    }

    // Reservation 已持有后再通知前端：先作废积压事件并释放 WebView2 的 DLC
    // 图片/音频句柄。Windows 不允许重命名仍被播放器占用的目录。
    app.emit("script:prepare-uninstall", folder_name.clone())
        .map_err(|error| format!("通知前端释放 DLC 资源失败: {error}"))?;
    #[cfg(target_os = "windows")]
    tokio::time::sleep(std::time::Duration::from_millis(1_300)).await;

    // Cleanup ownership is derivable from the sanitized install path, so even
    // malformed or too-new story_config/dlc.json content cannot block removal.
    let owner = Path::new("standalone")
        .join(&folder_name)
        .to_string_lossy()
        .to_string();
    let data_dir = state.ai_service.lock().await.data_dir.clone();
    // The same-volume rename is the uninstall commit point. Before it, package
    // and progress are untouched; after it, the package is outside every scan
    // root and a durable transaction can finish cleanup after any hard kill.
    let transaction =
        crate::ai_service::game_system::script_engine::dlc_transaction::stage_uninstall(
            &data_dir, &target, &owner,
        )
        .map_err(|error| format!("隔离 DLC 目录失败，未改动周目状态: {error:#}"))?;

    {
        let mut service = state.ai_service.lock().await;
        service.script_manager.unload_script_dir(&target);
    }

    crate::ai_service::game_system::script_engine::dlc_transaction::finalize_uninstall(
        &data_dir,
        &transaction,
    )
    .map_err(|error| {
        format!("DLC 已安全移出运行目录，但清理尚未完成；下次启动会自动重试: {error:#}")
    })?;
    Ok(())
}

/// 在 zip 条目名列表里定位 story_config.yaml 的位置。
/// 返回根前缀："" = 平铺包（配置在 zip 根），"<壳目录>/" = 带壳包。
fn detect_script_root(names: &[String]) -> Result<String, String> {
    const CONFIG: &str = "story_config.yaml";
    if names.iter().any(|n| n == CONFIG) {
        return Ok(String::new());
    }
    let mut roots: Vec<String> = names
        .iter()
        .filter_map(|n| n.strip_suffix(CONFIG))
        .filter(|prefix| !prefix.is_empty() && prefix.matches('/').count() == 1)
        .map(|prefix| prefix.to_string())
        .collect();
    roots.sort();
    roots.dedup();
    match roots.len() {
        0 => Err("DLC 包里找不到 story_config.yaml，不是有效的剧本包".to_string()),
        1 => Ok(roots.remove(0)),
        _ => Err("DLC 包里有多个 story_config.yaml，无法识别主剧本".to_string()),
    }
}
