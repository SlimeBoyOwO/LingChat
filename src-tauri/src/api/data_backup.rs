//! 数据备份 / 恢复 Tauri 命令。
//!
//! 将数据库记录、Tauri 配置、前端偏好、资源文件等打包为单个 zip 备份文件，
//! 并支持从备份文件选择性恢复。
//!
//! 复用 `lan_sync::db_sync` 的 DB 导出/导入逻辑（`export_all_records` / `apply_db_records`）。

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;

use crate::ai_service::tts::local::saf_bridge::prepare_file_import_source;
use crate::api::game_data_dir;
use crate::config;
use crate::AppState;

/// 备份格式版本号。升级时只增不降，前端据此判断兼容性。
const BACKUP_VERSION: u32 = 1;

// ─── 请求 / 响应类型 ─────────────────────────────────────────

/// 用户选择要备份 / 恢复的内容。
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupSelections {
    pub database: bool,
    pub settings: bool,
    pub frontend_preferences: bool,
    pub characters: bool,
    pub backgrounds: bool,
    pub musics: bool,
    pub ambients: bool,
}

/// 备份清单（写入 zip 根目录的 `manifest.json`）。
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    /// 备份格式版本号。
    pub version: u32,
    /// 导出时间戳（Unix 毫秒）。
    pub exported_at: u64,
    /// 导出的 LingChat 版本号。
    pub app_version: String,
    /// 包含的内容。
    pub selections: BackupSelectionsInfo,
}

/// 备份包含的内容清单。
#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct BackupSelectionsInfo {
    pub database: bool,
    pub settings: bool,
    pub frontend_preferences: bool,
    pub characters: bool,
    pub backgrounds: bool,
    pub musics: bool,
    pub ambients: bool,
}

/// 导入结果，返回给前端。
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// 前端偏好 JSON（由前端写回 Pinia）。
    pub frontend_preferences_json: Option<String>,
    /// 是否恢复了数据库。
    pub database_imported: bool,
    /// 是否恢复了设置。
    pub settings_imported: bool,
    /// 恢复了哪些资源目录。
    pub files_restored: Vec<String>,
    /// 是否需要重启应用以加载新数据（恢复数据库或设置后为 true）。
    pub needs_restart: bool,
}

// ─── Tauri 命令 ──────────────────────────────────────────────

/// 导出数据备份到指定路径。
///
/// 前端传入 `frontend_preferences_json`（Pinia persist 序列化后的 JSON 字符串），
/// 后端将其打包进 zip。`dest_path` 支持桌面文件路径和 Android SAF 内容 URI。
#[tauri::command]
pub async fn export_data_backup(
    app: AppHandle,
    selections: BackupSelections,
    frontend_preferences_json: Option<String>,
    dest_path: String,
) -> Result<(), String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cache dir: {e}"))?;
    let temp_dir = cache_dir.join(format!("backup_export_{}", std::process::id()));
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("create temp dir: {e}"))?;

    let data_dir = crate::api::data_dir();
    let game_data_dir = game_data_dir();

    // 1. 清单
    let manifest = BackupManifest {
        version: BACKUP_VERSION,
        exported_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        selections: BackupSelectionsInfo {
            database: selections.database,
            settings: selections.settings,
            frontend_preferences: selections.frontend_preferences,
            characters: selections.characters,
            backgrounds: selections.backgrounds,
            musics: selections.musics,
            ambients: selections.ambients,
        },
    };

    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("serialize manifest: {e}"))?;
    tokio::fs::write(temp_dir.join("manifest.json"), &manifest_json)
        .await
        .map_err(|e| format!("write manifest: {e}"))?;

    // 2. 数据库记录
    if selections.database {
        tracing::info!("[DataBackup] 导出数据库记录...");
        let records = crate::lan_sync::db_sync::export_all_records(&data_dir).await?;
        let json = serde_json::to_string_pretty(&records)
            .map_err(|e| format!("serialize db records: {e}"))?;
        tokio::fs::write(temp_dir.join("db_records.json"), &json)
            .await
            .map_err(|e| format!("write db records: {e}"))?;
    }

    // 3. Tauri 配置 (settings.json)
    if selections.settings {
        tracing::info!("[DataBackup] 导出 Tauri 配置...");
        let map = export_settings_to_map(&app)?;
        let json = serde_json::to_string_pretty(&map)
            .map_err(|e| format!("serialize settings: {e}"))?;
        tokio::fs::write(temp_dir.join("settings.json"), &json)
            .await
            .map_err(|e| format!("write settings: {e}"))?;
    }

    // 4. 前端偏好
    if let Some(prefs) = &frontend_preferences_json {
        tokio::fs::write(temp_dir.join("frontend_preferences.json"), prefs)
            .await
            .map_err(|e| format!("write frontend preferences: {e}"))?;
    }

    // 5. 资源文件
    let resources_dir = temp_dir.join("resources");
    tokio::fs::create_dir_all(&resources_dir)
        .await
        .map_err(|e| format!("create resources dir: {e}"))?;

    let dirs: [(bool, &str, PathBuf); 4] = [
        (selections.characters, "characters", game_data_dir.join("characters")),
        (
            selections.backgrounds,
            "backgrounds",
            game_data_dir.join("backgrounds"),
        ),
        (selections.musics, "musics", game_data_dir.join("musics")),
        (
            selections.ambients,
            "ambients",
            game_data_dir.join("ambients"),
        ),
    ];

    for (enabled, name, src) in dirs {
        if enabled && src.exists() {
            let dest = resources_dir.join(name);
            tokio::fs::create_dir_all(&dest)
                .await
                .map_err(|e| format!("create {} dir: {e}", name))?;
            let src_path = src.clone();
            let dest_path = dest.clone();
            tokio::task::spawn_blocking(move || copy_dir_recursive_sync(&src_path, &dest_path))
                .await
                .map_err(|e| format!("spawn_blocking copy {name}: {e}"))?
                .map_err(|e| format!("copy {}: {e}", name))?;
        }
    }

    // 6. 打包为 zip
    let zip_filename = format!("lingchat_backup_{}.zip", manifest.exported_at);
    let zip_path = cache_dir.join(&zip_filename);
    let temp_dir_for_zip = temp_dir.clone();
    let zip_path_clone = zip_path.clone();
    tokio::task::spawn_blocking(move || create_zip_from_dir(&temp_dir_for_zip, &zip_path_clone))
        .await
        .map_err(|e| format!("spawn_blocking zip: {e}"))?
        .map_err(|e| format!("create zip: {e}"))?;

    tracing::info!(
        "[DataBackup] zip 已创建: {} ({:.1} MB)",
        zip_path.display(),
        std::fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0) as f64 / 1048576.0
    );

    // 7. 复制到目标路径
    copy_to_destination(&app, &zip_path, &dest_path).await?;

    // 8. 清理临时文件
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    let _ = tokio::fs::remove_file(&zip_path).await;

    tracing::info!("[DataBackup] 导出完成: dest={}", dest_path);
    Ok(())
}

/// 读取备份文件的清单（不恢复，仅预览）。
///
/// 前端在导入前先调用此命令查看备份内容，让用户选择要恢复的部分。
/// 支持桌面文件路径和 Android SAF content:// URI。
#[tauri::command]
pub async fn peek_data_backup(app: AppHandle, src_path: String) -> Result<BackupManifest, String> {
    let source = prepare_file_import_source(&app, &src_path).await?;
    let path = source.path;
    let cleanup_saf_cache = source.cleanup_after_import;

    let result = (|| {
        let file = File::open(&path).map_err(|e| format!("打开备份文件失败: {e}"))?;
        let mut archive =
            zip::read::ZipArchive::new(file).map_err(|e| format!("读取压缩包失败: {e}"))?;
        let mut manifest_file = archive
            .by_name("manifest.json")
            .map_err(|e| format!("找不到 manifest.json: {e}"))?;
        let mut json = String::new();
        manifest_file
            .read_to_string(&mut json)
            .map_err(|e| format!("读取 manifest: {e}"))?;
        let manifest: BackupManifest =
            serde_json::from_str(&json).map_err(|e| format!("解析 manifest: {e}"))?;

        if manifest.version > BACKUP_VERSION {
            return Err(format!(
                "备份文件版本 ({}) 高于当前支持的版本 ({})",
                manifest.version, BACKUP_VERSION
            ));
        }

        Ok(manifest)
    })();

    // 清理 SAF 临时缓存文件
    if cleanup_saf_cache {
        if let Err(e) = tokio::fs::remove_file(&path).await {
            tracing::warn!("[DataBackup] 清理 SAF 临时文件失败: {e}");
        }
    }

    result
}

/// 从备份文件恢复数据。
///
/// 返回 `ImportResult`，其中包含前端偏好 JSON（由前端写回 Pinia）和恢复结果摘要。
/// 支持桌面文件路径和 Android SAF content:// URI。
#[tauri::command]
pub async fn import_data_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    src_path: String,
    selections: BackupSelections,
) -> Result<ImportResult, String> {
    // 处理 Android SAF content:// URI
    let source = prepare_file_import_source(&app, &src_path).await?;
    let local_path = source.path;
    let cleanup_saf_cache = source.cleanup_after_import;

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cache dir: {e}"))?;
    let temp_dir = cache_dir.join(format!("backup_import_{}", std::process::id()));
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    tokio::fs::create_dir_all(&temp_dir)
        .await
        .map_err(|e| format!("create temp dir: {e}"))?;

    // 1. 解压到临时目录
    let src_str = local_path.to_string_lossy().into_owned();
    let dst_path = temp_dir.clone();
    tokio::task::spawn_blocking(move || extract_zip_to_dir_sync(&src_str, &dst_path))
        .await
        .map_err(|e| format!("spawn_blocking extract: {e}"))?
        .map_err(|e| format!("解压失败: {e}"))?;

    // 2. 读取清单
    let manifest_json = tokio::fs::read_to_string(temp_dir.join("manifest.json"))
        .await
        .map_err(|e| format!("读取 manifest: {e}"))?;
    let manifest: BackupManifest =
        serde_json::from_str(&manifest_json).map_err(|e| format!("解析 manifest: {e}"))?;

    let game_data_dir = game_data_dir();
    let mut result = ImportResult {
        frontend_preferences_json: None,
        database_imported: false,
        settings_imported: false,
        files_restored: Vec::new(),
        needs_restart: false,
    };

    // 3. 恢复数据库
    if selections.database && manifest.selections.database {
        tracing::info!("[DataBackup] 恢复数据库记录...");
        let db_json = tokio::fs::read_to_string(temp_dir.join("db_records.json"))
            .await
            .map_err(|e| format!("读取 db_records.json: {e}"))?;
        let records: crate::lan_sync::messages::DbRecords =
            serde_json::from_str(&db_json).map_err(|e| format!("解析 db records: {e}"))?;
        let db = state.db.clone();
        crate::lan_sync::db_sync::apply_db_records(&db, &records).await?;
        result.database_imported = true;
        result.needs_restart = true;
    }

    // 4. 恢复设置
    if selections.settings && manifest.selections.settings {
        tracing::info!("[DataBackup] 恢复 Tauri 配置...");
        let settings_json = tokio::fs::read_to_string(temp_dir.join("settings.json"))
            .await
            .map_err(|e| format!("读取 settings.json: {e}"))?;
        let map: serde_json::Value =
            serde_json::from_str(&settings_json).map_err(|e| format!("解析 settings: {e}"))?;
        import_settings_from_map(&app, &map)?;
        result.settings_imported = true;
        result.needs_restart = true;
    }

    // 5. 前端偏好（返回给前端）
    if selections.frontend_preferences && manifest.selections.frontend_preferences {
        if let Ok(prefs) = tokio::fs::read_to_string(temp_dir.join("frontend_preferences.json")).await
        {
            result.frontend_preferences_json = Some(prefs);
        }
    }

    // 6. 资源文件
    let resources_dir = temp_dir.join("resources");
    if resources_dir.exists() {
        for (enabled, name, dest_dir) in [
            (selections.characters, "characters", game_data_dir.join("characters")),
            (selections.backgrounds, "backgrounds", game_data_dir.join("backgrounds")),
            (selections.musics, "musics", game_data_dir.join("musics")),
            (selections.ambients, "ambients", game_data_dir.join("ambients")),
        ] {
            let src = resources_dir.join(name);
            if enabled && src.exists() {
                tokio::fs::create_dir_all(&dest_dir)
                    .await
                    .map_err(|e| format!("create {} dir: {e}", name))?;
                let src_path = src.clone();
                let dest_path = dest_dir.clone();
                tokio::task::spawn_blocking(move || copy_dir_recursive_sync(&src_path, &dest_path))
                    .await
                    .map_err(|e| format!("spawn_blocking copy {name}: {e}"))?
                    .map_err(|e| format!("restore {}: {e}", name))?;
                result.files_restored.push(name.to_string());
            }
        }
    }

    // 7. 清理临时文件
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    if cleanup_saf_cache {
        let _ = tokio::fs::remove_file(&local_path).await;
    }


    tracing::info!("[DataBackup] 导入完成: {:?}", result);
    Ok(result)
}

// ─── 设置导入 / 导出 ─────────────────────────────────────────

/// 读取 Tauri settings store 的全部键值，序列化为 JSON 对象。
fn export_settings_to_map(app: &AppHandle) -> Result<serde_json::Value, String> {
    let store = config::settings_store(app).map_err(|e| e.to_string())?;
    let keys = store.keys();
    let mut map = serde_json::Map::new();
    for key in keys {
        if let Some(value) = store.get(&key) {
            map.insert(key, value);
        }
    }
    Ok(serde_json::Value::Object(map))
}

/// 从 JSON 对象写回 Tauri settings store。
fn import_settings_from_map(app: &AppHandle, map: &serde_json::Value) -> Result<(), String> {
    let store = config::settings_store(app).map_err(|e| e.to_string())?;
    let obj = map
        .as_object()
        .ok_or_else(|| "settings 不是 JSON 对象".to_string())?;
    for (key, value) in obj {
        store.set(key.clone(), value.clone());
    }
    store.save().map_err(|e| format!("保存配置失败: {e}"))?;
    Ok(())
}

// ─── 文件辅助 ────────────────────────────────────────────────

/// 同步递归复制目录。
fn copy_dir_recursive_sync(src: &Path, dest: &Path) -> Result<(), String> {
    if !dest.exists() {
        std::fs::create_dir_all(dest)
            .map_err(|e| format!("创建目标目录 {}: {e}", dest.display()))?;
    }
    for entry in std::fs::read_dir(src).map_err(|e| format!("读取源目录: {e}"))? {
        let entry = entry.map_err(|e| format!("读取目录项: {e}"))?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive_sync(&path, &target)?;
        } else if path.is_file() {
            std::fs::copy(&path, &target)
                .map_err(|e| format!("复制文件 {}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// 同步创建 zip（递归目录下所有文件）。
fn create_zip_from_dir(dir: &Path, zip_path: &Path) -> Result<(), String> {
    let file = File::create(zip_path).map_err(|e| format!("创建 zip 文件: {e}"))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(5));

    let mut files = Vec::new();
    collect_files(dir, &mut files)?;

    for path in &files {
        let rel = path.strip_prefix(dir).unwrap_or(path);
        let name = rel.to_string_lossy().replace('\\', "/");
        zip.start_file(&name, options)
            .map_err(|e| format!("zip start_file: {e}"))?;
        let mut f = File::open(path).map_err(|e| format!("打开文件 {}: {e}", path.display()))?;
        std::io::copy(&mut f, &mut zip)
            .map_err(|e| format!("写入 zip {}: {e}", path.display()))?;
    }

    zip.finish().map_err(|e| format!("完成 zip: {e}"))?;
    Ok(())
}

/// 递归收集目录下所有文件路径。
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| format!("读取目录 {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("读取目录项: {e}"))?;
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        } else if path.is_dir() {
            collect_files(&path, out)?;
        }
    }
    Ok(())
}

/// 同步解压 zip 到指定目录。
///
/// 采用多层路径遍历防护：
/// 1. 拒绝包含 `..` 的条目名
/// 2. 拒绝以 `/` 或 `\` 开头的绝对路径
/// 3. 拒绝 Windows 盘符路径（如 `C:/evil.txt`）
/// 4. 规范化后校验最终路径仍在目标目录内
fn extract_zip_to_dir_sync(zip_path: &str, dest_dir: &Path) -> Result<(), String> {
    let file = File::open(zip_path).map_err(|e| format!("打开 zip: {e}"))?;
    let mut archive =
        zip::read::ZipArchive::new(file).map_err(|e| format!("读取 zip: {e}"))?;

    // 预先规范化目标目录，用于后续校验
    let canonical_base = dest_dir
        .canonicalize()
        .or_else(|_| {
            // 目录可能不存在，先创建再规范化
            std::fs::create_dir_all(dest_dir).ok();
            dest_dir.canonicalize()
        })
        .map_err(|e| format!("目标目录校验失败: {e}"))?;

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("读取条目 {i}: {e}"))?;
        let name = entry.name().to_string();

        // 安全检查 1：拒绝路径遍历
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            return Err(format!("路径遍历攻击: {name}"));
        }

        // 安全检查 2：拒绝 Windows 盘符路径（如 C:/evil.txt 或 D:\evil.txt）
        if name.len() >= 2 {
            let bytes = name.as_bytes();
            if bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                return Err(format!("路径遍历攻击: {name}"));
            }
        }

        let dest_path = dest_dir.join(&name);

        // 安全检查 3：规范化后校验仍在目标目录内
        // 文件尚未创建，先规范化其父目录
        let canonical_dest = if let Some(parent) = dest_path.parent() {
            let canonical_parent = parent.canonicalize().unwrap_or_else(|_| parent.to_path_buf());
            canonical_parent.join(dest_path.file_name().unwrap_or_default())
        } else {
            dest_path.clone()
        };
        if !canonical_dest.starts_with(&canonical_base) {
            return Err(format!("路径遍历攻击: {name}"));
        }

        if entry.is_dir() {
            std::fs::create_dir_all(&dest_path)
                .map_err(|e| format!("创建目录 {name}: {e}"))?;
        } else {
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("创建父目录 {name}: {e}"))?;
            }
            let mut out = File::create(&dest_path)
                .map_err(|e| format!("创建文件 {name}: {e}"))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("写入文件 {name}: {e}"))?;
        }
    }

    Ok(())
}

/// 复制 zip 到目标路径（桌面端原生复制 / Android SAF）。
async fn copy_to_destination(
    app: &AppHandle,
    src: &Path,
    dest_path: &str,
) -> Result<(), String> {
    if dest_path.starts_with("content://") {
        use tauri_plugin_android_fs::{AndroidFsExt, FsUri};

        let source_uri = FsUri::from_path(src);
        let destination_uri = FsUri::from_uri(dest_path.to_string());
        tracing::info!(
            "[DataBackup] SAF 复制: {} -> {}",
            src.display(),
            dest_path
        );
        app.android_fs_async()
            .copy(&source_uri, &destination_uri)
            .await
            .map_err(|e| format!("复制到 SAF 目标: {e}"))?;
    } else {
        let dest = PathBuf::from(dest_path);
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| format!("创建目标父目录: {e}"))?;
        }
        tokio::fs::copy(src, &dest)
            .await
            .map_err(|e| format!("复制到目标: {e}"))?;
    }
    Ok(())
}
