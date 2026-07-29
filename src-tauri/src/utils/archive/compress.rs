//! 把目录压缩为 zip / 7z 归档（导出功能）。
//!
//! - [`compress`] 公共入口
//! - `collect_files` 私有：递归收集文件（私有 helper）
//! - `compress_zip` 私有：zip 实现
//! - `compress_sevenz` 私有：7z 实现
//!
//! macOS metadata（`__MACOSX/`、`._*`、`.DS_Store`）会被自动跳过。

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::{map_sevenz_err, ArchiveError, ArchiveFormat, EntryEvent};

pub fn compress(
    src_dir: &Path,
    format: ArchiveFormat,
    out: &Path,
    on_entry: &dyn Fn(EntryEvent),
) -> Result<(), ArchiveError> {
    match format {
        ArchiveFormat::Zip => compress_zip(src_dir, out, on_entry),
        ArchiveFormat::SevenZ => compress_sevenz(src_dir, out, on_entry),
    }
}

// 递归收集目录下所有文件，跳过 macOS metadata。
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            out.push(path);
        } else if path.is_dir() {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            if name == "__MACOSX" || name.starts_with("._") {
                continue;
            }
            collect_files(&path, out)?;
        }
    }
    Ok(())
}

// ZIP 压缩实现：使用 `zip` crate 的 `SimpleFileOptions`，deflate level 5。
fn compress_zip(
    src_dir: &Path,
    out: &Path,
    on_entry: &dyn Fn(EntryEvent),
) -> Result<(), ArchiveError> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let file = File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(5));

    let mut files = Vec::new();
    collect_files(src_dir, &mut files)?;
    let total = files.len();

    on_entry(EntryEvent {
        phase: "started",
        total,
        ..Default::default()
    });

    for (i, path) in files.iter().enumerate() {
        let rel = path.strip_prefix(src_dir).unwrap_or(path);
        let name = rel.to_string_lossy().replace('\\', "/");
        if name.starts_with("._") || name.contains("/._") || name == ".DS_Store"
            || name.ends_with("/.DS_Store")
        {
            continue;
        }
        zip.start_file(&name, options)?;
        let mut f = File::open(path)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        zip.write_all(&buf)?;

        on_entry(EntryEvent {
            phase: "entry",
            index: i + 1,
            total,
            name,
            bytes_done: buf.len() as u64,
            bytes_entry: buf.len() as u64,
            ..Default::default()
        });
    }

    zip.finish()?;
    on_entry(EntryEvent {
        phase: "finished",
        index: total,
        total,
        ..Default::default()
    });
    Ok(())
}

// 7z 压缩实现：使用 `sevenz_rust2::ArchiveWriter`。
fn compress_sevenz(
    src_dir: &Path,
    out: &Path,
    on_entry: &dyn Fn(EntryEvent),
) -> Result<(), ArchiveError> {
    use sevenz_rust2::{ArchiveEntry, ArchiveWriter};

    if let Some(parent) = out.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    let file = File::create(out)?;
    let mut writer = ArchiveWriter::new(file).map_err(map_sevenz_err)?;

    let mut files = Vec::new();
    collect_files(src_dir, &mut files)?;
    let total = files.len();
    on_entry(EntryEvent {
        phase: "started",
        total,
        ..Default::default()
    });

    for (i, path) in files.iter().enumerate() {
        let rel = path.strip_prefix(src_dir).unwrap_or(path);
        let name = rel.to_string_lossy().replace("\\", "/");
        if name.starts_with("._") || name.contains("/._") || name == ".DS_Store"
            || name.ends_with("/.DS_Store")
        {
            continue;
        }
        let entry = ArchiveEntry::from_path(path, name.clone());
        writer
            .push_archive_entry(entry, Some(File::open(path)?))
            .map_err(map_sevenz_err)?;
        on_entry(EntryEvent {
            phase: "entry",
            index: i + 1,
            total,
            name,
            bytes_done: 0,
            bytes_entry: 0,
            ..Default::default()
        });
    }
    writer.finish()?;
    on_entry(EntryEvent {
        phase: "finished",
        index: total,
        total,
        ..Default::default()
    });
    Ok(())
}