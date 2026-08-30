//! 插件压缩包导入：解压 → 严格定位 → manifest 校验 → 落位 → 重扫注册。
//!
//! 与角色导入（`api::role_archive::import_pipeline`）共用 `utils::archive` 的解压、
//! 冲突解析与目录搬运实现，两处刻意不同：
//!
//! 1. **校验在落位之前**。角色流程是「先 rename 进目标位、再查 `settings.yml`、
//!    失败删目标位」；插件在 `Overwrite` 下照抄会先把用户的好插件删掉，才发现新包
//!    是坏的。这里所有内容根、manifest、脚本存在性的判定都发生在 staging 内。
//! 2. **目标目录名取 `manifest.id`**，不取压缩包文件名。`manager.rs` 强制
//!    `manifest.id == 目录名`，因此冲突策略只接受 `Overwrite` / `Skip`（放弃），
//!    没有 `Rename` 的容身之处。
//! 3. **落位后默认禁用**。覆盖时走 `delete_plugin` 清掉旧记录与集中状态条目，
//!    新插件以「未启用」的全新姿态出现，由用户在插件页手动开启。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

use crate::utils::archive::{self, ArchiveError, ArchiveFormat, ConflictPolicy, EntryEvent};

use super::manifest;
use super::types::PluginManifest;

/// 导入成功的结果（供前端进度条与插件页刷新使用）。
#[derive(Debug, Clone, serde::Serialize)]
pub struct PluginImportResult {
    pub plugin_id: String,
    pub plugin_name: String,
    pub version: String,
    /// `"created"` / `"overwritten"`（`Skip` 命中已存在时直接返回错误，不会出现在这里）。
    pub conflict_action: String,
    /// 该插件声明携带的资源类型。
    pub resources: Vec<String>,
    pub bytes_extracted: u64,
    pub warnings: Vec<String>,
    /// 后端 magic 决定的真实格式。
    pub format: ArchiveFormat,
}

/// 压缩包结构判定失败的原因，映射为前端 i18n 错误码。
enum LayoutError {
    /// 两层深度内找不到任何 manifest.toml。
    MissingManifest,
    /// 找到了 manifest.toml，但它所在的位置不符合「根」或「唯一一层子目录」。
    BadLayout,
}

/// 导入一个插件压缩包。`policy` 只接受 [`ConflictPolicy::Overwrite`] 与
/// [`ConflictPolicy::Skip`]；调用方（`api::plugins::import_plugin_from_path`）负责拒绝其他值。
pub async fn do_import_plugin(
    app: &AppHandle,
    tmp_path: &Path,
    format_hint: Option<ArchiveFormat>,
    policy: ConflictPolicy,
    cancel_token: Arc<CancellationToken>,
) -> Result<PluginImportResult, String> {
    // 1. 以文件头魔数决定真实格式，前端 hint 只用于日志比对。
    let detected = archive::detect_format(tmp_path).map_err(|e| e.to_string())?;
    let format = match format_hint {
        Some(hint) if hint == detected => detected,
        Some(hint) => {
            tracing::warn!(
                "[PluginImport] 扩展名 hint={hint:?} 与 magic {detected:?} 不一致，采用 magic 结果"
            );
            detected
        }
        None => detected,
    };

    // 2. 在插件根目录下建暂存区。点开头 + manager 扫描已跳过隐藏目录，
    //    半途失败的垃圾不会被当成插件加载。
    let plugins_root = plugin_root_dir();
    tokio::fs::create_dir_all(&plugins_root)
        .await
        .map_err(|e| format!("创建 plugins dir: {e}"))?;
    let staging_root = plugins_root.join(format!(".import_staging_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&staging_root)
        .await
        .map_err(|e| format!("创建 staging dir: {e}"))?;

    let staging_for_cleanup = staging_root.clone();
    let cleanup = |p: &Path| {
        let _ = std::fs::remove_dir_all(p);
    };

    // 3. 解压到暂存目录，逐条目推送进度。
    let app_emit = app.clone();
    let path_for_blocking = tmp_path.to_path_buf();
    let cancel_for_blocking = cancel_token.clone();
    let target = staging_root.clone();
    let summary = tokio::task::spawn_blocking(move || {
        // on_entry 不检查 cancel：extract_zip/extract_sevenz 在每条 entry 前已检查
        // cancel_token 并直接返回 ArchiveError::Cancelled，不会调到这里。
        let on_entry = |evt: EntryEvent| {
            let _ = app_emit.emit("plugin:import-progress", &evt);
        };
        match format {
            ArchiveFormat::Zip => {
                archive::extract_zip(&path_for_blocking, &target, &cancel_for_blocking, &on_entry)
            }
            ArchiveFormat::SevenZ => archive::extract_sevenz(
                &path_for_blocking,
                &target,
                &cancel_for_blocking,
                &on_entry,
            ),
        }
    })
    .await
    .map_err(|e| {
        cleanup(&staging_for_cleanup);
        format!("spawn_blocking join: {e}")
    })?
    .map_err(|e| {
        tracing::error!("[PluginImport] 解压失败: {e}");
        cleanup(&staging_for_cleanup);
        e.to_string()
    })?;

    if cancel_token.is_cancelled() {
        cleanup(&staging_for_cleanup);
        return Err("导入已取消".into());
    }

    // 4. 严格定位内容根目录（见函数文档的两种形态）。
    let extracted_dir = match locate_plugin_root(&staging_root).await {
        Ok(dir) => dir,
        Err(err) => {
            cleanup(&staging_for_cleanup);
            return Err(match err {
                LayoutError::MissingManifest => "PLUGIN_MISSING_MANIFEST".into(),
                LayoutError::BadLayout => "PLUGIN_BAD_ARCHIVE_LAYOUT".into(),
            });
        }
    };

    // 5. 解析并校验 manifest。格式错误、未知字段、id 非法字符都归到同一个错误码。
    let manifest = match read_manifest(&extracted_dir).await {
        Ok(m) => m,
        Err(e) => {
            tracing::error!("[PluginImport] manifest 校验失败: {e}");
            cleanup(&staging_for_cleanup);
            return Err("PLUGIN_INVALID_MANIFEST".into());
        }
    };
    // id 已由 manifest::validate 限定为 [A-Za-z0-9_-]，可直接作为目录名，无穿越风险。
    let plugin_id = manifest.id.clone();

    // 6. tools 声明的每个脚本必须真实存在。
    if let Some(missing) = find_missing_script(&extracted_dir, &manifest).await {
        tracing::error!("[PluginImport] 工具脚本缺失: {missing}");
        cleanup(&staging_for_cleanup);
        return Err(format!("PLUGIN_MISSING_TOOL_SCRIPT|{missing}"));
    }

    if cancel_token.is_cancelled() {
        cleanup(&staging_for_cleanup);
        return Err("导入已取消".into());
    }

    // 7. 解析目标位置。Skip 命中已存在 → 报冲突（插件没有「跳过并成功」的语义，
    //    装了个重复东西却提示成功会让用户以为覆盖生效了）。
    let resolution = match archive::resolve_target(&plugins_root, &plugin_id, policy) {
        Ok(r) => r,
        Err(ArchiveError::AlreadyExists(name)) => {
            cleanup(&staging_for_cleanup);
            return Err(format!("PLUGIN_ALREADY_EXISTS|{name}"));
        }
        Err(e) => {
            cleanup(&staging_for_cleanup);
            return Err(e.to_string());
        }
    };

    // 8. 覆盖：先经 manager.delete_plugin 注销工具并删除旧目录与状态条目。
    //    直接 remove_dir_all 会把已注册的工具留在 registry 里，重启前一直可调。
    if resolution.action == "overwritten" {
        let manager = app.state::<crate::AppState>().data().plugin_manager.clone();
        match manager.delete_plugin(&plugin_id).await {
            Ok(()) => {}
            // 目标目录存在但 manager 里没有对应记录（例如手工丢进去、manifest 损坏的
            // 残留目录），退化为直接删目录。
            Err(e) => {
                tracing::warn!("[PluginImport] delete_plugin 未命中，退化为删目录: {e}");
                if let Err(err) = tokio::fs::remove_dir_all(&resolution.target).await {
                    cleanup(&staging_for_cleanup);
                    return Err(format!(
                        "无法覆盖已存在的插件目录 {} (可能正在被使用, 请重启后重试): {err}",
                        resolution.target.display()
                    ));
                }
            }
        }
    }

    // 9. 落位。
    if let Err(e) = archive::relocate_dir(&extracted_dir, &resolution.target).await {
        tracing::error!("[PluginImport] 移动目录失败: {e}");
        cleanup(&staging_for_cleanup);
        return Err(e);
    }
    // 内容位于暂存目录根部时上一步已把暂存目录整体改名，此处只清空壳。
    let _ = tokio::fs::remove_dir_all(&staging_root).await;

    // 10. 重扫并收敛派生状态（插件角色入库 / 剧本合并 / 背景场景化）。
    let manager = app.state::<crate::AppState>().data().plugin_manager.clone();
    tokio::task::spawn_blocking(move || manager.reload())
        .await
        .map_err(|e| format!("插件重载线程异常: {e}"))?;
    crate::api::plugins::refresh_plugin_content(app).await;

    tracing::info!(
        "[PluginImport] 完成: id={plugin_id}, action={}, files={}, bytes={}",
        resolution.action,
        summary.files_extracted,
        summary.bytes_extracted
    );

    Ok(PluginImportResult {
        plugin_id: plugin_id.clone(),
        plugin_name: manifest.name.clone(),
        version: manifest.version.clone(),
        conflict_action: resolution.action.into(),
        resources: manifest
            .resources
            .iter()
            .map(|k| k.as_str().to_string())
            .collect(),
        bytes_extracted: summary.bytes_extracted,
        warnings: summary.warnings,
        format,
    })
}

/// `data/plugins` 根目录（与 `PluginManager::new` 的定位方式一致）。
pub fn plugin_root_dir() -> PathBuf {
    crate::init::static_copy::get_data_dir().join("plugins")
}

/// 严格定位插件内容根目录，只接受两种形态：
///
/// - **A**：`manifest.toml` 直接位于暂存目录根部（压缩包内平铺文件与资源）。
/// - **B**：`manifest.toml` 位于唯一一层子目录内（压缩包外套了一个文件夹）。
///   沿用角色导入的判定条件：子目录唯一且根部没有任何散落文件，
///   因此 `README.md + my-plugin/manifest.toml` 这类混合形态会被拒绝。
async fn locate_plugin_root(staging: &Path) -> Result<PathBuf, LayoutError> {
    if staging.join("manifest.toml").is_file() {
        return Ok(staging.to_path_buf());
    }
    let (subdirs, has_files) = archive::list_content_entries(staging).await;
    if subdirs.len() == 1 && !has_files && subdirs[0].join("manifest.toml").is_file() {
        return Ok(subdirs[0].clone());
    }
    // 区分两种失败：两层深度内根本没有 manifest.toml → 缺文件；
    // 有但位置不合规（多个子目录 / 根部混文件 / 嵌更深）→ 结构不支持。
    let manifest_anywhere = subdirs
        .iter()
        .take(64)
        .any(|child| child.join("manifest.toml").is_file());
    Err(if manifest_anywhere {
        LayoutError::BadLayout
    } else {
        LayoutError::MissingManifest
    })
}

/// 读取并校验 staging 内的 manifest.toml（复用插件加载期的同一套解析与校验规则）。
async fn read_manifest(dir: &Path) -> anyhow::Result<PluginManifest> {
    let text = tokio::fs::read_to_string(dir.join("manifest.toml"))
        .await
        .map_err(|e| anyhow::anyhow!("读取 manifest.toml 失败: {e}"))?;
    Ok(manifest::parse(&text)?)
}

/// 返回第一个「manifest 声明了但实际不存在」的工具脚本名。
async fn find_missing_script(dir: &Path, manifest: &PluginManifest) -> Option<String> {
    for tool in &manifest.tools {
        // manifest::validate 已保证 script 是单个文件名，这里只判存在性。
        if !dir.join(&tool.script).is_file() {
            return Some(tool.script.clone());
        }
    }
    None
}
