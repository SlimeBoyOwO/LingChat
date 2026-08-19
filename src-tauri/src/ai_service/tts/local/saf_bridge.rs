//! Platform-specific bridge for preparing an import source path for local TTS.
//!
//! On desktop the user's path is returned as-is; on Android we transparently
//! stage a `content://`-prefixed path into the app cache.

use std::path::PathBuf;
use tauri::AppHandle;

#[cfg(target_os = "android")]
use tauri::Manager;

/// Prepare the actual on-disk source for a local TTS import.
///
/// Returns `(path_to_import_from, should_cleanup_after_import)`. When
/// `should_cleanup_after_import` is `true` the caller MUST delete the staged
/// file once it has finished processing.
pub async fn prepare_file_import_source(
    app: &AppHandle,
    path: &str,
) -> Result<(PathBuf, bool), String> {
    if path.starts_with("content://") {
        #[cfg(target_os = "android")]
        {
            use tauri_plugin_android_fs::{AndroidFsExt, FsUri};
            let cache_dir = app
                .path()
                .app_cache_dir()
                .map_err(|e| format!("cache dir: {e}"))?;
            let imports_root = cache_dir.join("imports");
            tokio::fs::create_dir_all(&imports_root)
                .await
                .map_err(|e| format!("create imports dir: {e}"))?;
            let tmp_id = uuid::Uuid::new_v4().to_string();
            let src_uri = FsUri::from_uri(path.to_string());

            let display = app
                .android_fs_async()
                .get_name_or_last_path_segment(&src_uri)
                .await;
            let suffix =
                sanitize_staged_filename(&display).unwrap_or_else(|| "import.bin".to_string());

            let local_path = imports_root.join(format!("tts_import_saf_{tmp_id}_{suffix}"));
            let local_uri = FsUri::from_path(&local_path);
            tracing::info!(
                "[tts_local] prepare_file_import_source SAF: src={}, local={}",
                path,
                local_path.display()
            );
            app.android_fs_async()
                .copy(&src_uri, &local_uri)
                .await
                .map_err(|e| format!("SAF copy to local cache: {e}"))?;
            return Ok((local_path, true));
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            Err("content URI imports are only supported on Android".into())
        }
    } else {
        Ok((PathBuf::from(path), false))
    }
}

/// Strip path separators and characters that are illegal on FAT/NTFS or
/// could cause traversal problems. Returns `None` if the cleaned result is
/// empty (caller should fall back to a default extension).
#[allow(dead_code)] // only invoked from the `#[cfg(target_os = "android")]` arm above
fn sanitize_staged_filename(raw: &str) -> Option<String> {
    let basename = raw
        .rsplit_once(['/', '\\'])
        .map(|(_, name)| name)
        .unwrap_or(raw);
    let cleaned: String = basename
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        .collect();
    let trimmed = cleaned
        .trim()
        .trim_matches(|c| matches!(c, '.' | '\u{ff0e}' | '\u{2024}' | '\u{fe52}'))
        .to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}
