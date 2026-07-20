//! 角色压缩包 (zip / 7z) 解压/压缩统一接口。
//!
//! # 解压安全防线
//!
//! 仅检查总大小无法识别 ZIP 炸弹。类似 42.zip、35.zip 的递归压缩包中，
//! 每个条目都可能具有极高压缩率，解压与压缩大小之比可超过 1000。
//!
//! 1. **条目数量** — `entry_index < 1000`
//! 2. **压缩比** — `uncompressed / compressed <= 100` ← 关键防线
//!
//! # 路径遍历防御
//! 任何条目名包含 `..`、以 `/` 或 `\` 开头、Windows 盘符或 UNC 路径时，
//! 直接拒绝。同时跳过 macOS 元数据 (`__MACOSX/`、`._*`、`.DS_Store`)。

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tokio_util::sync::CancellationToken;

// ===== 安全阈值常量 =====

pub const MAX_ENTRY_COUNT: usize = 1000;
pub const MAX_COMPRESSION_RATIO: u64 = 100;
const MAX_NAME_LEN: usize = 4096;

// ===== 类型 =====

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    Zip,
    SevenZ,
}

impl ArchiveFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZ => "7z",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    Skip,
    Rename,
    Overwrite,
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("不支持的压缩包格式: {0}")]
    UnsupportedFormat(String),
    #[error("路径遍历攻击: {0}")]
    PathTraversal(String),
    #[error("非法文件名: {0}")]
    InvalidName(String),
    #[error("entry 数量超限: {0} 个, 限制 {MAX_ENTRY_COUNT}")]
    TooManyEntries(usize),
    #[error("压缩比超限 (解压/压缩 > {MAX_COMPRESSION_RATIO}): 解压 {actual} 字节, 压缩 {compressed} 字节")]
    CompressionRatio { actual: u64, compressed: u64 },
    #[error("zip 错误: {0}")]
    Zip(String),
    #[error("7z 错误: {0}")]
    SevenZ(String),
    #[error("IO 错误: {0}")]
    Io(#[from] io::Error),
    #[error("密码保护的压缩包暂不支持")]
    PasswordProtected,
    #[error("操作被取消")]
    Cancelled,
    #[error("目标已存在: {0}")]
    AlreadyExists(String),
}

impl From<zip::result::ZipError> for ArchiveError {
    fn from(e: zip::result::ZipError) -> Self {
        match e {
            zip::result::ZipError::UnsupportedArchive(msg) if msg.contains("encrypted") => {
                Self::PasswordProtected
            }
            _ => Self::Zip(e.to_string()),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct EntryEvent {
    #[serde(rename = "phase")]
    pub phase: &'static str, // "started" | "entry" | "finished" | "error"
    pub index: usize,
    pub total: usize,
    pub name: String,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub bytes_entry: u64,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ExtractSummary {
    pub bytes_extracted: u64,
    pub files_extracted: usize,
    pub skipped_macos_metadata: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
pub struct TargetResolution {
    pub target: PathBuf,
    pub final_name: String,
    pub action: &'static str, // "created" | "renamed" | "overwritten"
}

// ===== 1. 文件头魔数检测 =====

pub const ZIP_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
const ZIP_EMPTY_MAGIC: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
pub const SEVENZ_MAGIC: [u8; 6] = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];

pub fn detect_format(path: &Path) -> Result<ArchiveFormat, ArchiveError> {
    let mut f = File::open(path)?;
    let mut buf = [0u8; 6];
    let n = f.read(&mut buf)?;
    if n < 4 {
        return Err(ArchiveError::UnsupportedFormat(format!("文件过小 ({} 字节)", n)));
    }
    if buf[..4] == ZIP_MAGIC || buf[..4] == ZIP_EMPTY_MAGIC {
        return Ok(ArchiveFormat::Zip);
    }
    if n >= 6 && buf == SEVENZ_MAGIC {
        return Ok(ArchiveFormat::SevenZ);
    }
    Err(ArchiveError::UnsupportedFormat(format!(
        "未知 magic: {:02X?}",
        &buf[..n.min(6)]
    )))
}

// ===== 2. 安全检查 (P0 #4 修复核心) =====

/// 单个条目解压前的安全检查。
///
/// 返回 `Ok` 表示条目可以解压，返回 `Err` 表示必须立即终止解压流程。
///
/// # 防线 (按检查顺序)
/// 1. 条目数量小于 `MAX_ENTRY_COUNT`
/// 2. **压缩比 <= MAX_COMPRESSION_RATIO** (解压后/压缩前)
///    压缩大小为零时跳过压缩比检查，例如仅存储条目或异常元数据。
pub fn check_entry_safety(
    entry_index: usize,
    entry_compressed: u64,
    entry_uncompressed: u64,
) -> Result<(), ArchiveError> {
    if entry_index >= MAX_ENTRY_COUNT {
        return Err(ArchiveError::TooManyEntries(entry_index));
    }
    if entry_compressed > 0 && entry_uncompressed / entry_compressed > MAX_COMPRESSION_RATIO {
        return Err(ArchiveError::CompressionRatio {
            actual: entry_uncompressed,
            compressed: entry_compressed,
        });
    }
    Ok(())
}

// ===== 3. 路径清洗与安全拼接 =====

/// 清洗条目名称并拒绝危险路径。
///
/// 拒绝: 空名、含 `..` 组件、绝对路径 (Unix `/` / Windows 盘符 / UNC `\\`)、
/// macOS 元数据 (`__MACOSX/`、`._*`、`.DS_Store`)、过长文件名。
///
/// 清洗: 控制字符和 Windows 保留字符 `\ : * ? " < > |` 替换为 `_`。
pub fn sanitize_entry_name(raw: &str) -> Result<String, ArchiveError> {
    if raw.is_empty() {
        return Err(ArchiveError::InvalidName("空文件名".into()));
    }
    if raw.len() > MAX_NAME_LEN {
        return Err(ArchiveError::InvalidName(format!(
            "文件名过长 ({} 字节, 限制 {})",
            raw.len(),
            MAX_NAME_LEN
        )));
    }
    if is_macos_metadata(raw) {
        return Err(ArchiveError::InvalidName(format!("macOS 元数据: {raw}")));
    }
    if raw.split(['/', '\\']).any(|s| s == "..") {
        return Err(ArchiveError::PathTraversal(format!("\"..\" 组件: {raw}")));
    }
    if raw.starts_with('/') {
        return Err(ArchiveError::PathTraversal(format!("Unix 绝对路径: {raw}")));
    }
    let bytes = raw.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/') {
        return Err(ArchiveError::PathTraversal(format!("Windows 盘符: {raw}")));
    }
    if raw.starts_with("\\\\") {
        return Err(ArchiveError::PathTraversal(format!("UNC 路径: {raw}")));
    }
    let cleaned: String = raw
        .chars()
        .map(|c| match c {
            '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    if cleaned.split(['/', '\\']).any(|s| s == "..") {
        return Err(ArchiveError::PathTraversal(format!("清洗后仍含 ..: {cleaned}")));
    }
    Ok(cleaned)
}

fn is_macos_metadata(name: &str) -> bool {
    name == "__MACOSX"
        || name.starts_with("__MACOSX/")
        || name.starts_with("__MACOSX\\")
        || name.contains("/__MACOSX/")
        || name == ".DS_Store"
        || name.ends_with("/.DS_Store")
        || name.ends_with("\\.DS_Store")
        || name.starts_with("._")
        || name.contains("/._")
}

pub fn safe_join(dest_root: &Path, cleaned_name: &str) -> Result<PathBuf, ArchiveError> {
    let out = dest_root.join(cleaned_name);
    if !out.starts_with(dest_root) {
        return Err(ArchiveError::PathTraversal(format!(
            "路径逃逸: {cleaned_name} -> {}",
            out.display()
        )));
    }
    Ok(out)
}

// ===== 4. 解析目标目录（冲突策略） =====

pub fn resolve_target(
    base: &Path,
    preferred: &str,
    policy: ConflictPolicy,
) -> Result<TargetResolution, ArchiveError> {
    let target = base.join(preferred);
    if !target.exists() {
        return Ok(TargetResolution {
            target,
            final_name: preferred.into(),
            action: "created",
        });
    }
    match policy {
        ConflictPolicy::Skip => Err(ArchiveError::AlreadyExists(preferred.into())),
        ConflictPolicy::Overwrite => Ok(TargetResolution {
            target,
            final_name: preferred.into(),
            action: "overwritten",
        }),
        ConflictPolicy::Rename => {
            for n in 2..=999 {
                let name = format!("{preferred}_{n}");
                let candidate = base.join(&name);
                if !candidate.exists() {
                    return Ok(TargetResolution {
                        target: candidate,
                        final_name: name,
                        action: "renamed",
                    });
                }
            }
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let name = format!("{preferred}_{ts}");
            Ok(TargetResolution {
                target: base.join(&name),
                final_name: name,
                action: "renamed",
            })
        }
    }
}

// ===== 5. 解压 =====

pub fn extract_zip(
    src: &Path,
    dest_root: &Path,
    cancel_token: &CancellationToken,
    on_entry: &dyn Fn(EntryEvent),
) -> Result<ExtractSummary, ArchiveError> {
    use zip::ZipArchive;

    let file = File::open(src)?;
    let mut archive = ZipArchive::new(file)?;
    let total = archive.len();

    on_entry(EntryEvent {
        phase: "started",
        total,
        ..Default::default()
    });

    let mut bytes_done: u64 = 0;
    let mut summary = ExtractSummary::default();
    let mut last_emit = std::time::Instant::now();

    for i in 0..total {
        if cancel_token.is_cancelled() {
            return Err(ArchiveError::Cancelled);
        }
        let mut entry = archive.by_index(i)?;
        let raw_name = entry.name().to_string();
        let compressed = entry.compressed_size();
        let uncompressed = entry.size();

        // 写入文件前执行条目安全检查。
        check_entry_safety(i, compressed, uncompressed)?;

        let cleaned = match sanitize_entry_name(&raw_name) {
            Ok(c) => c,
            Err(ArchiveError::InvalidName(msg)) => {
                summary.skipped_macos_metadata += 1;
                summary.warnings.push(msg);
                continue;
            }
            Err(e) => return Err(e),
        };
        let out_path = safe_join(dest_root, &cleaned)?;

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
            continue;
        }
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let bytes_written = io::copy(&mut entry, &mut File::create(&out_path)?)?;
        bytes_done += bytes_written;
        summary.files_extracted += 1;

        // 进度事件最短间隔为 80 毫秒；小型压缩包仍逐条发送。
        let elapsed = last_emit.elapsed();
        if elapsed >= std::time::Duration::from_millis(80)
            || total < 100
            || i + 1 == total
        {
            on_entry(EntryEvent {
                phase: "entry",
                index: i + 1,
                total,
                name: raw_name,
                bytes_done,
                bytes_entry: bytes_written,
                ..Default::default()
            });
            last_emit = std::time::Instant::now();
        }
    }

    on_entry(EntryEvent {
        phase: "finished",
        index: total,
        total,
        bytes_done,
        ..Default::default()
    });

    summary.bytes_extracted = bytes_done;
    Ok(summary)
}

/// 解压 7z 压缩包。
/// 与 ZIP 解压保持相同的取消检查、条目安全检查、路径清洗和进度节流策略。
pub fn extract_sevenz(
    src: &Path,
    dest_root: &Path,
    cancel_token: &CancellationToken,
    on_entry: &dyn Fn(EntryEvent),
) -> Result<ExtractSummary, ArchiveError> {
    use sevenz_rust2::Password;

    let file = File::open(src)?;
    let mut reader = sevenz_rust2::ArchiveReader::new(file, Password::empty())
        .map_err(map_sevenz_err)?;
    let total = reader.archive().files.len();

    on_entry(EntryEvent {
        phase: "started",
        total,
        ..Default::default()
    });

    let mut bytes_done: u64 = 0;
    let mut summary = ExtractSummary::default();
    let mut last_emit = std::time::Instant::now();
    let mut processed: usize = 0;
    let mut entry_index: usize = 0;

    // 第一遍处理带数据流的条目，解压器会按固实块顺序提供数据。
    let result = reader.for_each_entries(|entry, reader| {
        if cancel_token.is_cancelled() {
            return Err(sevenz_rust2::Error::Io(
                std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled"),
                "".into(),
            ));
        }
        let raw_name = entry.name().to_string();
        let uncompressed = entry.size();
        let compressed = entry.compressed_size;
        let cleaned = match sanitize_entry_name(&raw_name) {
            Ok(c) => c,
            Err(ArchiveError::InvalidName(msg)) => {
                summary.skipped_macos_metadata += 1;
                summary.warnings.push(msg);
                processed += 1;
                entry_index += 1;
                return Ok(true);
            }
            Err(e) => return Err(sevenz_rust2::Error::Io(std::io::Error::other(e.to_string()), "".into())),
        };
        if let Err(e) = check_entry_safety(entry_index, compressed, uncompressed) {
            return Err(sevenz_rust2::Error::Io(std::io::Error::other(e.to_string()), "".into()));
        }
        entry_index += 1;

        let out_path = match safe_join(dest_root, &cleaned) {
            Ok(p) => p,
            Err(e) => return Err(sevenz_rust2::Error::Io(std::io::Error::other(e.to_string()), "".into())),
        };

        if entry.is_directory() {
            std::fs::create_dir_all(&out_path).map_err(|e| sevenz_rust2::Error::Io(e, "".into()))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| sevenz_rust2::Error::Io(e, "".into()))?;
            }
            let mut f = File::create(&out_path).map_err(|e| sevenz_rust2::Error::Io(e, "".into()))?;
            if uncompressed > 0 {
                let n = io::copy(reader, &mut f).map_err(|e| sevenz_rust2::Error::Io(e, "".into()))?;
                bytes_done += n;
                summary.files_extracted += 1;
            }
        }

        let elapsed = last_emit.elapsed();
        if elapsed >= std::time::Duration::from_millis(80)
            || total < 100
            || processed + 1 == total
        {
            on_entry(EntryEvent {
                phase: "entry",
                index: processed + 1,
                total,
                name: raw_name,
                bytes_done,
                bytes_entry: uncompressed,
                ..Default::default()
            });
            last_emit = std::time::Instant::now();
        }
        processed += 1;
        Ok(true)
    });
    if let Err(ref e) = result {
        if cancel_token.is_cancelled() {
            return Err(ArchiveError::Cancelled);
        }
    }
    result.map_err(map_sevenz_err)?;

    // 第二遍补建没有数据流的目录和空文件。
    for (i, entry) in reader.archive().files.iter().enumerate() {
        if entry.is_anti_item {
            continue;
        }
        if entry.has_stream() {
            continue; // 已在第一遍处理。
        }
        let raw_name = entry.name.clone();
        let size = entry.size;
        let cleaned = match sanitize_entry_name(&raw_name) {
            Ok(c) => c,
            Err(ArchiveError::InvalidName(msg)) => {
                summary.skipped_macos_metadata += 1;
                summary.warnings.push(msg);
                continue;
            }
            Err(_) => continue,
        };
        if let Err(e) = check_entry_safety(entry_index, 0, size) {
            // 无数据流条目只可能触发条目数量限制，记录后跳过。
            tracing::warn!("7z empty entry skipped: {}", e);
            entry_index += 1;
            continue;
        }
        entry_index += 1;
        let out_path = match safe_join(dest_root, &cleaned) {
            Ok(p) => p,
            Err(_) => continue,
        };
        if entry.is_directory() || size == 0 {
            std::fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            let _ = File::create(&out_path);
        }
        let _ = i;
    }

    on_entry(EntryEvent {
        phase: "finished",
        index: total,
        total,
        bytes_done,
        ..Default::default()
    });
    summary.bytes_extracted = bytes_done;
    Ok(summary)
}

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

fn map_sevenz_err(e: sevenz_rust2::Error) -> ArchiveError {
    match e {
        sevenz_rust2::Error::PasswordRequired => ArchiveError::PasswordProtected,
        other => ArchiveError::SevenZ(other.to_string()),
    }
}

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
