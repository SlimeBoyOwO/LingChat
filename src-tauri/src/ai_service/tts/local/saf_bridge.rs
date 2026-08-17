//! Platform-specific bridge for preparing an import source path for local TTS.
//!
//! On desktop the user's path is returned as-is; on Android we transparently
//! stage a `content://`-prefixed path into the app cache.
//!
//! The returned `display_name` is the user-facing file name (e.g.
//! `"MyFont.woff2"`), derived from the original URI on Android and from
//! `path`'s file_name on desktop. Callers should use it as `original_name`
//! for UX/notification purposes — the staged `PathBuf` has a synthetic
//! `tts_import_saf_<uuid>_…` name on Android that must not leak into UI.

use std::path::{Path, PathBuf};
use tauri::AppHandle;

#[cfg(target_os = "android")]
use tauri::Manager;

/// User-facing metadata for an import source.
pub struct ImportSource {
    /// Local path to read bytes from.
    pub path: PathBuf,
    /// True when the caller must delete `path` after processing.
    pub cleanup_after_import: bool,
    /// Best-effort user-facing file name (e.g. `"MyFont.woff2"`).
    /// Falls back to the staged path's file name when the platform cannot
    /// recover the original.
    pub display_name: String,
}

/// Prepare the actual on-disk source for a local TTS import.
///
/// Returns an [`ImportSource`]. When `cleanup_after_import` is `true` the
/// caller MUST delete the staged file once it has finished processing.
pub async fn prepare_file_import_source(
    app: &AppHandle,
    path: &str,
) -> Result<ImportSource, String> {
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
                "[tts_local] prepare_file_import_source SAF: src={}, local={}, display={}",
                path,
                local_path.display(),
                display,
            );
            app.android_fs_async()
                .copy(&src_uri, &local_uri)
                .await
                .map_err(|e| format!("SAF copy to local cache: {e}"))?;
            return Ok(ImportSource {
                path: local_path,
                cleanup_after_import: true,
                display_name: if display.is_empty() {
                    "import.bin".to_string()
                } else {
                    display
                },
            });
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            Err("content URI imports are only supported on Android".into())
        }
    } else {
        // Desktop / non-content path: use the file's own basename as the
        // user-facing display name. Falls back to the full path string if
        // for some reason the path has no file name component.
        let pb = PathBuf::from(path);
        let display_name = pb
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| path.to_string());
        // Touch `Path` to keep the import meaningful on platforms where the
        // std-only branch is unused (lint hygiene for `Path` re-export above).
        let _ = Path::new(path);
        Ok(ImportSource {
            path: pb,
            cleanup_after_import: false,
            display_name,
        })
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

#[cfg(test)]
mod tests {
    use super::sanitize_staged_filename;

    #[test]
    fn strips_path_traversal() {
        assert_eq!(sanitize_staged_filename("../etc/passwd").unwrap(), "passwd");
        assert_eq!(
            sanitize_staged_filename("..\\evil.onnx").unwrap(),
            "evil.onnx"
        );
    }

    #[test]
    fn strips_control_chars() {
        assert_eq!(
            sanitize_staged_filename("\nvoice.onnx\r").unwrap(),
            "voice.onnx"
        );
    }

    #[test]
    fn strips_illegal_chars() {
        assert_eq!(sanitize_staged_filename("a:b*c?.onnx").unwrap(), "abc.onnx");
    }

    #[test]
    fn empty_inputs_become_none() {
        assert!(sanitize_staged_filename("").is_none());
        assert!(sanitize_staged_filename("///").is_none());
        assert!(sanitize_staged_filename("...").is_none());
        assert!(sanitize_staged_filename(" \t ").is_none());
    }

    #[test]
    fn rejects_unicode_dot_only_names() {
        assert!(sanitize_staged_filename("．．").is_none());
        assert!(sanitize_staged_filename("․․").is_none());
        assert!(sanitize_staged_filename("﹒﹒").is_none());
    }

    #[test]
    fn strips_unicode_dot_traversal_prefixes() {
        assert_eq!(
            sanitize_staged_filename("．．voice.onnx").unwrap(),
            "voice.onnx"
        );
        assert_eq!(
            sanitize_staged_filename("․․style_vectors.json").unwrap(),
            "style_vectors.json"
        );
    }

    #[test]
    fn keeps_common_extensions() {
        assert_eq!(
            sanitize_staged_filename("voice.onnx").unwrap(),
            "voice.onnx"
        );
        assert_eq!(
            sanitize_staged_filename("MyModel.sbv2").unwrap(),
            "MyModel.sbv2"
        );
        assert_eq!(
            sanitize_staged_filename("archive.zip").unwrap(),
            "archive.zip"
        );
        assert_eq!(sanitize_staged_filename("voice.7z").unwrap(), "voice.7z");
    }
}
