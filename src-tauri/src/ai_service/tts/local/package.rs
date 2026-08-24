// Inspect + install model packages. Supports raw SBV2/ONNX files and zip/7z
// archives containing those files. Extraction delegates to the shared
// `crate::utils::archive` module for safety (zip-bomb protection, path
// sanitization, cancellation).

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use super::paths::LocalTtsPaths;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageKind {
    RawSbv2,
    RawOnnx,
    Zip,
    SevenZ,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectedPackage {
    pub kind: PackageKind,
    pub file_name: String,
    pub size_bytes: u64,
    /// For archives: filename that looks like the model file inside.
    pub inner_model_name: Option<String>,
}

/// Cheap extension-first sniff.
pub fn detect_by_extension(path: &Path) -> PackageKind {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    match ext.as_str() {
        "sbv2" => PackageKind::RawSbv2,
        "onnx" => PackageKind::RawOnnx,
        "zip" => PackageKind::Zip,
        "7z" => PackageKind::SevenZ,
        _ => PackageKind::Unknown,
    }
}

/// Sniff archive format via infer crate (replaces handwritten magic bytes).
/// Falls back to Unknown for non-archive content.
pub fn detect_archive_by_infer(path: &Path) -> std::result::Result<PackageKind, String> {
    let kind = infer::get_from_path(path).map_err(|e| format!("infer: {e}"))?;
    Ok(match kind.map(|k| k.mime_type()) {
        Some("application/zip") | Some("application/x-zip-compressed") => PackageKind::Zip,
        Some("application/x-7z-compressed") => PackageKind::SevenZ,
        _ => PackageKind::Unknown,
    })
}

pub fn inspect_package(path: &Path) -> std::result::Result<InspectedPackage, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("metadata: {e}"))?;
    let size_bytes = meta.len();
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let kind = detect_by_extension(path);
    // 扩展名未知时用 infer 兜底（替代原 detect_by_magic）。
    let kind = if kind == PackageKind::Unknown {
        detect_archive_by_infer(path)?
    } else {
        kind
    };

    let inner_model_name = if matches!(kind, PackageKind::Zip | PackageKind::SevenZ) {
        scan_archive_for_model(path, kind).ok()
    } else {
        None
    };

    Ok(InspectedPackage {
        kind,
        file_name,
        size_bytes,
        inner_model_name,
    })
}

fn scan_archive_for_model(
    path: &Path,
    kind: PackageKind,
) -> std::result::Result<String, String> {
    let found = match kind {
        PackageKind::Zip => {
            let f = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
            let mut zip =
                zip::ZipArchive::new(f).map_err(|e| format!("zip: {e}"))?;
            let mut found: Option<String> = None;
            for i in 0..zip.len() {
                let entry =
                    zip.by_index(i).map_err(|e| format!("zip entry: {e}"))?;
                let n = entry.name().to_lowercase();
                if n.ends_with(".sbv2") || n.ends_with(".onnx") {
                    found = Some(entry.name().to_string());
                    break;
                }
            }
            found
        }
        PackageKind::SevenZ => {
            let f = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
            let archive = sevenz_rust2::ArchiveReader::new(
                f,
                sevenz_rust2::Password::empty(),
            )
            .map_err(|e| format!("7z: {e}"))?;
            let mut found: Option<String> = None;
            for entry in archive.archive().files.iter() {
                let n = entry.name().to_lowercase();
                if n.ends_with(".sbv2") || n.ends_with(".onnx") {
                    found = Some(entry.name().to_string());
                    break;
                }
            }
            found
        }
        _ => return Err("not an archive".into()),
    };
    found.ok_or_else(|| "archive does not contain a .sbv2 or .onnx file".to_string())
}

/// Install inspected package into the voice directory.
/// Uses the shared archive extraction utilities from `crate::utils::archive`
/// which include zip-bomb protection, path sanitization, and cancellation support.
pub fn install_inspected(
    inspected: &InspectedPackage,
    src: &Path,
    paths: &LocalTtsPaths,
    voice_id: &str,
) -> std::result::Result<PathBuf, String> {
    let dst = paths.voice_dir(voice_id);
    std::fs::create_dir_all(&dst).map_err(|e| format!("create voice dir: {e}"))?;

    match inspected.kind {
        PackageKind::RawSbv2 => crate::utils::fs::copy_with_parent(src, &dst.join("model.sbv2")),
        PackageKind::RawOnnx => crate::utils::fs::copy_with_parent(src, &dst.join("model.onnx")),
        PackageKind::Zip | PackageKind::SevenZ => {
            let token = std::sync::Arc::new(tokio_util::sync::CancellationToken::new());
            let src_buf = src.to_path_buf();
            let dst_buf = dst.clone();
            let kind = inspected.kind;
            let result: Result<crate::utils::archive::ExtractSummary, crate::utils::archive::ArchiveError> =
                tokio::task::block_in_place(|| match kind {
                    PackageKind::Zip => crate::utils::archive::extract_zip(
                        &src_buf,
                        &dst_buf,
                        &token,
                        &|_| {},
                    ),
                    PackageKind::SevenZ => crate::utils::archive::extract_sevenz(
                        &src_buf,
                        &dst_buf,
                        &token,
                        &|_| {},
                    ),
                    _ => unreachable!(),
                });
            result.map_err(|e| format!("extract: {e}"))?;
            for candidate in ["model.sbv2", "model.onnx"] {
                let p = dst.join(candidate);
                if p.exists() {
                    return Ok(p);
                }
            }
            Err("extracted archive does not contain model.sbv2 or model.onnx".into())
        }
        PackageKind::Unknown => Err("unknown package format".into()),
    }
}
