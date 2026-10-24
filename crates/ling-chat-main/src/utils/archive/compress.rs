//! 把目录压缩为 zip / 7z 归档（导出功能）。
//!
//! 两种入口：
//! - [`compress`]：通用压缩，支持把内存条目（`extra_entries`）一并写入，并可按
//!   **归档内条目名**（相对 `src_dir`、`/` 分隔，含子目录路径）排除磁盘文件
//! - [`compress_excluding_root_entries`]：只按**第一层名字**排除，不改动嵌套内容
//!
//! macOS metadata（`__MACOSX/`、`._*`、`.DS_Store`）一律自动跳过。

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use super::{ArchiveError, ArchiveFormat, EntryEvent, map_sevenz_err};

/// 压缩 `src_dir` 到 `out`，并把 `extra_entries` 作为内存条目一并写入归档。
///
/// `extra_entries` 的 `(条目名, 内容)` 与磁盘文件同名时**以内存条目为准**（磁盘那份被
/// 剔除）；`exclude_names` 里的磁盘文件一律不进归档（调用方按需传入，本模块不关心语义）。
/// 两者的条目名都与磁盘文件的相对路径同口径：相对 `src_dir`、`/` 分隔。
pub fn compress(
    src_dir: &Path,
    format: ArchiveFormat,
    out: &Path,
    extra_entries: &[(String, Vec<u8>)],
    exclude_names: &[&str],
    on_entry: &dyn Fn(EntryEvent),
) -> Result<(), ArchiveError> {
    let mut files = Vec::new();
    collect_files(src_dir, &mut files, &[])?;
    // 内存条目与磁盘文件同名时剔除磁盘那份，另外剔除排除名单里的文件。
    // 放在 `retain` 而非逐条目 `continue`，让 `total` 与实际写出的条目数一致。
    files.retain(|path| should_pack(path, src_dir, extra_entries, exclude_names));
    compress_files(src_dir, format, out, &files, extra_entries, on_entry)
}

/// 仅排除源目录第一层的指定条目，不改变通用压缩或同名嵌套资源的行为。
pub fn compress_excluding_root_entries(
    src_dir: &Path,
    format: ArchiveFormat,
    out: &Path,
    on_entry: &dyn Fn(EntryEvent),
    excluded: &[&str],
) -> Result<(), ArchiveError> {
    let mut files = Vec::new();
    collect_files(src_dir, &mut files, excluded)?;
    compress_files(src_dir, format, out, &files, &[], on_entry)
}

// 递归收集目录下所有文件。`excluded` 只作用于**本层**（递归时清空），因此调用方
// 传进来的名字只能排除源目录第一层的条目——这是与 `compress` 的 `exclude_names`
// （按完整相对路径匹配）的区别。
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>, excluded: &[&str]) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if excluded.contains(&name.as_ref())
            || name == "__MACOSX"
            || name.starts_with("._")
            || name == ".DS_Store"
        {
            continue;
        }
        if path.is_file() {
            out.push(path);
        } else if path.is_dir() {
            collect_files(&path, out, &[])?;
        }
    }
    Ok(())
}

// 磁盘文件在归档内的条目名：相对 `src_dir`、`/` 分隔。
fn entry_name(path: &Path, src_dir: &Path) -> String {
    let rel = path.strip_prefix(src_dir).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

// 磁盘文件是否该写进归档：被内存条目顶掉、或命中排除名单的都不写。
// 两者都精确匹配 `entry_name`（含子目录路径），不做后缀/子串匹配。
fn should_pack(
    path: &Path,
    src_dir: &Path,
    extra_entries: &[(String, Vec<u8>)],
    exclude_names: &[&str],
) -> bool {
    let name = entry_name(path, src_dir);
    !extra_entries.iter().any(|(extra, _)| *extra == name)
        && !exclude_names.contains(&name.as_str())
}

// 按已筛好的文件表写归档：要不要内存条目、怎么排除，都由调用方在进来之前定好。
fn compress_files(
    src_dir: &Path,
    format: ArchiveFormat,
    out: &Path,
    files: &[PathBuf],
    extra_entries: &[(String, Vec<u8>)],
    on_entry: &dyn Fn(EntryEvent),
) -> Result<(), ArchiveError> {
    match format {
        ArchiveFormat::Zip => compress_zip(src_dir, out, extra_entries, files, on_entry),
        ArchiveFormat::SevenZ => compress_sevenz(src_dir, out, extra_entries, files, on_entry),
    }
}

// ZIP 压缩实现：使用 `zip` crate 的 `SimpleFileOptions`，deflate level 5。
fn compress_zip(
    src_dir: &Path,
    out: &Path,
    extra_entries: &[(String, Vec<u8>)],
    files: &[PathBuf],
    on_entry: &dyn Fn(EntryEvent),
) -> Result<(), ArchiveError> {
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    let file = File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .compression_level(Some(5));

    let total = files.len() + extra_entries.len();

    on_entry(EntryEvent {
        phase: "started",
        total,
        ..Default::default()
    });

    for (i, path) in files.iter().enumerate() {
        let name = entry_name(path, src_dir);
        if name.starts_with("._")
            || name.contains("/._")
            || name == ".DS_Store"
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

    for (j, (name, bytes)) in extra_entries.iter().enumerate() {
        zip.start_file(name, options)?;
        zip.write_all(bytes)?;
        on_entry(EntryEvent {
            phase: "entry",
            index: files.len() + j + 1,
            total,
            name: name.clone(),
            bytes_done: bytes.len() as u64,
            bytes_entry: bytes.len() as u64,
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
    extra_entries: &[(String, Vec<u8>)],
    files: &[PathBuf],
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

    let total = files.len() + extra_entries.len();
    on_entry(EntryEvent {
        phase: "started",
        total,
        ..Default::default()
    });

    for (i, path) in files.iter().enumerate() {
        let name = entry_name(path, src_dir);
        if name.starts_with("._")
            || name.contains("/._")
            || name == ".DS_Store"
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

    for (j, (name, bytes)) in extra_entries.iter().enumerate() {
        let entry = ArchiveEntry::new_file(name);
        writer
            .push_archive_entry(entry, Some(std::io::Cursor::new(bytes.as_slice())))
            .map_err(map_sevenz_err)?;
        on_entry(EntryEvent {
            phase: "entry",
            index: files.len() + j + 1,
            total,
            name: name.clone(),
            bytes_done: bytes.len() as u64,
            bytes_entry: bytes.len() as u64,
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
