// Inspect + install model packages. Supports raw SBV2/ONNX files and zip/7z
// archives containing those files. Reuses `utils::archive` for extraction
// when needed.

use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

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

const MAGIC_PK: &[u8; 4] = b"PK\x03\x04";
const MAGIC_7Z: &[u8; 2] = &[0x37, 0x7A];

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

/// Sniff by magic bytes; falls back to Unknown if not a recognised archive.
pub fn detect_by_magic(bytes: &[u8]) -> PackageKind {
    if bytes.len() >= 4 && &bytes[..4] == MAGIC_PK {
        PackageKind::Zip
    } else if bytes.len() >= 2 && &bytes[..2] == MAGIC_7Z {
        PackageKind::SevenZ
    } else {
        PackageKind::Unknown
    }
}

pub fn inspect_package(path: &Path) -> std::result::Result<InspectedPackage, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("metadata: {e}"))?;
    let size_bytes = meta.len();
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let kind = detect_by_extension(path);
    let kind = if kind == PackageKind::Unknown {
        let mut head = vec![0u8; 8.min(size_bytes as usize)];
        if !head.is_empty() {
            use std::io::Read;
            std::fs::File::open(path)
                .and_then(|mut f| f.read_exact(&mut head))
                .map_err(|e| format!("read head: {e}"))?;
        }
        detect_by_magic(&head)
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
            // Cheap entry-name scan via the in-memory archive header; we
            // don't decompress here. Matches the zip path's read-only
            // iteration.
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

/// Install inspected package into the voice directory. `voice_id` is
/// required for raw files; for archives the inner model file is extracted
/// into `dst/<voice_id>/`.
pub fn install_inspected(
    inspected: &InspectedPackage,
    src: &Path,
    paths: &super::paths::LocalTtsPaths,
    voice_id: &str,
) -> std::result::Result<PathBuf, String> {
    let dst = paths.voice_dir(voice_id);
    std::fs::create_dir_all(&dst).map_err(|e| format!("create voice dir: {e}"))?;

    match inspected.kind {
        PackageKind::RawSbv2 => {
            let target = dst.join("model.sbv2");
            std::fs::copy(src, &target).map_err(|e| format!("copy sbv2: {e}"))?;
            Ok(target)
        }
        PackageKind::RawOnnx => {
            let target = dst.join("model.onnx");
            std::fs::copy(src, &target).map_err(|e| format!("copy onnx: {e}"))?;
            Ok(target)
        }
        PackageKind::Zip | PackageKind::SevenZ => {
            let token =
                std::sync::Arc::new(tokio_util::sync::CancellationToken::new());
            let on_entry = |_evt: super::zip_extract::EntryEvent| {};
            let src_buf = src.to_path_buf();
            let dst_buf = dst.clone();
            let token_clone = token.clone();
            let kind = inspected.kind;
            let _result: super::zip_extract::ExtractSummary =
                tokio::task::block_in_place(|| match kind {
                    PackageKind::Zip => {
                        super::zip_extract::extract_zip(
                            &src_buf,
                            &dst_buf,
                            &token_clone,
                            &on_entry,
                        )
                    }
                    PackageKind::SevenZ => {
                        super::zip_extract::extract_sevenz(
                            &src_buf,
                            &dst_buf,
                            &token_clone,
                            &on_entry,
                        )
                    }
                    _ => unreachable!(),
                })
                .map_err(|e| format!("extract: {e}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_sbv2_by_extension() {
        assert_eq!(
            detect_by_extension(Path::new("a.sbv2")),
            PackageKind::RawSbv2
        );
    }

    #[test]
    fn detect_zip_by_magic() {
        let mut head = Vec::from(MAGIC_PK.as_slice());
        head.extend_from_slice(&[0; 32]);
        assert_eq!(detect_by_magic(&head), PackageKind::Zip);
    }

    #[test]
    fn detect_unknown_when_garbage() {
        assert_eq!(detect_by_magic(&[1, 2, 3]), PackageKind::Unknown);
    }

    #[test]
    fn inspect_raw_sbv2() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.sbv2");
        std::fs::write(&p, b"fake").unwrap();
        let i = inspect_package(&p).unwrap();
        assert_eq!(i.kind, PackageKind::RawSbv2);
        assert_eq!(i.size_bytes, 4);
    }

    #[test]
    fn inspect_zip_with_sbv2_inside() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("voice.zip");
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(f);
            zip.start_file("model.sbv2", SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"abc").unwrap();
            zip.finish().unwrap();
        }
        let i = inspect_package(&zip_path).unwrap();
        assert_eq!(i.kind, PackageKind::Zip);
        assert_eq!(i.inner_model_name.as_deref(), Some("model.sbv2"));
    }
}
