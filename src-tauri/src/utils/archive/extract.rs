//! 解压入口：把 zip / 7z 归档解到目标目录。
//!
//! 安全策略（取消、条目数、压缩比、路径清洗、子树校验）与 [`archive.md`](../../../../docs/utils/archive.md) 一致。
//!
//! - [`extract_zip`] zip 路径（`zip` crate）
//! - [`extract_sevenz`] 7z 路径（`sevenz_rust2`）

use std::fs::File;
use std::io;
use std::path::Path;

use tokio_util::sync::CancellationToken;

use super::{map_sevenz_err, ArchiveError, EntryEvent, ExtractSummary};
use super::safety::{check_entry_safety, safe_join, sanitize_entry_name};

// ===== 5. 解压 =====

/// 解压 ZIP 归档到指定目录。
///
/// # 流程（每条目）
/// 1. `cancel_token.is_cancelled()` 取消闸门
/// 2. [`check_entry_safety`] 条目数/压缩比检查
/// 3. [`sanitize_entry_name`] 路径清洗（macOS metadata 作 warning 跳过）
/// 4. [`safe_join`] 目标根目录子树二次校验
/// 5. 写入文件（自动 `create_dir_all` 父目录）
/// 6. 80ms 节流推送 [`EntryEvent`]；总条目数 < 100 时逐条触发
///
/// # 参数
/// - `src`：磁盘上的 zip 文件路径
/// - `dest_root`：解压目标根目录（不存在会自动创建）
/// - `cancel_token`：异步取消令牌（每个条目写入前检查）
/// - `on_entry`：单条目进度回调
///
/// # 返回
/// [`ExtractSummary`] 统计结果；失败返回 [`ArchiveError`]。
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

/// 解压 7z 归档到指定目录。
///
/// 安全策略（取消、条目数、压缩比、路径清洗、子树校验）与 [`extract_zip`] 完全一致。
///
/// 7z 解压器需要分两遍：固实块顺序的数据流条目先建文件，再遍历建无流目录与空文件。
/// 7z 取消通过把 [`ArchiveError::Cancelled`] 映射为 `io::ErrorKind::Interrupted` 后抛回闭包，
/// 外层再捕获并转换。
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
    for entry in reader.archive().files.iter() {
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