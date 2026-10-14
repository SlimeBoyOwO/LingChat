//! 角色压缩包导出的内部实现：压缩角色目录到临时文件，并清洗用户输入的文件名。
//!
//! `compress_role_to_temp` 由 `mod.rs` 中的 `export_role_to_path` 调用。

use std::path::{Path, PathBuf};

use sea_orm::{DatabaseConnection, EntityTrait};
use tauri::{AppHandle, Emitter, Manager};

use crate::db::entities::role::Entity as RoleEntity;
use crate::utils::archive::{self, ArchiveFormat, EntryEvent};

use super::HOTWORD_SIDECAR_NAME;

// ===== 导出命令 =====

/// 导出内核：压缩 `src_dir` 到 `out`。
///
/// `_db`/`_role_id` 入参保留只为不动调用点（热词旁挂已停用，spec §3.7.1）。
/// 不依赖 `AppHandle`（`tauri/test` 未启用，取不到 mock）：路径解析与进度 emit 都留给
/// 外层 [`compress_role_to_temp`]，这里只认显式入参与回调，便于端到端测试。
async fn compress_role_dir<F: Fn(EntryEvent) + Send + 'static>(
    _db: &DatabaseConnection,
    _role_id: i32,
    src_dir: &Path,
    out: &Path,
    format: ArchiveFormat,
    on_entry: F,
) -> Result<(), String> {
    // 热词旁挂已停用（spec §3.7.1：热词是「人用什么词」的属性，不随角色包移动）。
    // 但老包导入后旁挂文件会留在角色目录里（导入侧有意不删），仍须按名排除——
    // 否则它会被原样打回新包，在旧版客户端上重新入库。编辑器草稿/回收站同理，
    // 它们只服务于本地编辑流程，不属于角色内容。
    //
    // 用 `compress_excluding_root_entries`（按第一层名字匹配）而不是带 `exclude_names`
    // 的通用 `compress`：后者按**归档内完整相对路径**精确匹配，是给 `extra_entries`
    // 那套「同名内存条目顶掉磁盘文件」用的，语义不同。
    let excluded: [&str; 3] = [HOTWORD_SIDECAR_NAME, ".editor-drafts", ".editor-trash"];

    let src_path = src_dir.to_path_buf();
    let arc_path = out.to_path_buf();
    tokio::task::spawn_blocking(move || {
        archive::compress_excluding_root_entries(&src_path, format, &arc_path, &on_entry, &excluded)
    })
    .await
    .map_err(|e| format!("spawn_blocking join: {e}"))?
    .map_err(|e| e.to_string())
}

/// 把角色压缩到缓存目录，并返回临时路径、建议文件名和文件大小。
pub(super) async fn compress_role_to_temp(
    app: &AppHandle,
    role_id: i32,
    format: ArchiveFormat,
) -> Result<(PathBuf, String, u64), String> {
    tracing::info!(
        "[RoleArchive] compress_role_to_temp 开始: role_id={}, format={:?}",
        role_id,
        format
    );
    let db = app.state::<crate::AppState>().db.clone();

    let role = RoleEntity::find_by_id(role_id)
        .one(&db)
        .await
        .map_err(|e| format!("query role: {e}"))?
        .ok_or_else(|| format!("role #{role_id} not found"))?;

    let folder = role
        .resource_folder
        .clone()
        .ok_or_else(|| format!("role #{role_id} has no resource_folder"))?;

    let characters_root = crate::api::characters_dir();
    let src_dir = characters_root.join(&folder);
    if !src_dir.is_dir() {
        return Err(format!("role folder not found: {}", src_dir.display()));
    }

    let cache_dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("cache dir: {e}"))?;
    let exports_root = cache_dir.join("exports");
    tokio::fs::create_dir_all(&exports_root)
        .await
        .map_err(|e| format!("create exports dir: {e}"))?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let safe_name = sanitize_file_name(&role.name);
    let suggested_name = format!("{safe_name}_{ts}.{}", format.as_str());
    let out_path = exports_root.join(&suggested_name);

    let app_for_emit = app.clone();
    compress_role_dir(&db, role_id, &src_dir, &out_path, format, move |evt| {
        // 发送导出进度事件，前端可复用现有进度条展示。
        let _ = app_for_emit.emit("role:export-progress", &evt);
    })
    .await?;

    let metadata = tokio::fs::metadata(&out_path)
        .await
        .map_err(|e| format!("stat output: {e}"))?;

    tracing::info!(
        "[RoleArchive] compress_role_to_temp 完成: temp_path={}, suggested_name={}, size={}B ({}MB)",
        out_path.display(),
        suggested_name,
        metadata.len(),
        metadata.len() / 1024 / 1024
    );

    Ok((out_path, suggested_name, metadata.len()))
}

fn sanitize_file_name(name: &str) -> String {
    let chars: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = chars.trim();
    if trimmed.is_empty() {
        "role".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::path::Path;

    /// 临时角色目录；调用方持有 `TempDir`，drop 时自动清理。
    fn role_dir(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        for (name, content) in files {
            std::fs::write(dir.path().join(name), content).expect("写角色文件失败");
        }
        dir
    }

    /// 归档输出路径。必须落在 src_dir 之外，否则压缩包会把自己也打进去。
    fn out_path(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("创建临时目录失败");
        let path = dir.path().join(name);
        (dir, path)
    }

    /// 读回归档条目（名, 内容），用于断言注入结果与重复条目。
    fn zip_entries(path: &Path) -> Vec<(String, Vec<u8>)> {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(path).expect("打开压缩包失败"))
            .expect("解析 zip 失败");
        (0..zip.len())
            .map(|i| {
                let mut entry = zip.by_index(i).expect("读取条目失败");
                let mut buf = Vec::new();
                entry.read_to_end(&mut buf).expect("读取条目内容失败");
                (entry.name().to_string(), buf)
            })
            .collect()
    }

    /// 角色目录里若已有同名文件（上次导入的残留、用户手放），注入时必须剔除磁盘那份，
    /// 否则包内会出现两个同名条目。
    #[tokio::test]
    async fn export_excludes_disk_file_with_same_name() {
        let injected = br#"{"injected":true}"#.to_vec();
        let src = role_dir(&[
            ("settings.yml", "ai_name: 钦灵\n"),
            ("conflict.txt", "{\"disk\":true}"),
        ]);
        let (_out_dir, out) = out_path("role.zip");

        archive::compress(
            src.path(),
            ArchiveFormat::Zip,
            &out,
            &[("conflict.txt".to_string(), injected.clone())],
            &[],
            &|_| {},
        )
        .expect("压缩失败");

        let dupes: Vec<(String, Vec<u8>)> = zip_entries(&out)
            .into_iter()
            .filter(|(n, _)| n == "conflict.txt")
            .collect();
        assert_eq!(dupes.len(), 1, "同名条目必须只出现一份");
        assert_eq!(dupes[0].1, injected, "保留的应是内存注入的那份");
    }
}
