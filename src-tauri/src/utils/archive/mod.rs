//! 角色压缩包 (zip / 7z) 解压/压缩统一接口。
//!
//! 配套架构说明位于 `docs/utils/archive.md`，本文件聚焦公共 API 的契约。
//!
//! # 解压安全防线
//!
//! 仅检查总大小无法识别 ZIP 炸弹。类似 42.zip、35.zip 的递归压缩包中，
//! 每个条目都可能具有极高压缩率，解压与压缩大小之比可超过 1000。
//!
//! 1. **条目数量** — `entry_index < MAX_ENTRY_COUNT`（默认 1000）
//! 2. **压缩比** — `uncompressed / compressed <= MAX_COMPRESSION_RATIO`（默认 100）← 关键防线
//!
//! # 路径遍历防御
//!
//! 任何条目名包含 `..`、以 `/` 或 `\` 开头、Windows 盘符或 UNC 路径时，
//! 直接拒绝。同时跳过 macOS 元数据 (`__MACOSX/`、`._*`、`.DS_Store`)。
//!
//! # 调用示例
//!
//! ```ignore
//! use ling_chat::utils::archive::{compress, ArchiveFormat, extract_zip, resolve_target, ConflictPolicy};
//! //!
//! compress(src_dir, ArchiveFormat::Zip, &out_zip, &|ev| {
//!     println!("[{} {}/{}] {}", ev.phase, ev.index, ev.total, ev.name);
//! })?;
//!
//! let token = CancellationToken::new();
//! let summary = extract_zip(&out_zip, &dest, &token, &|ev| { /* ... */ })?;
//! let res = resolve_target(&dest, &archive_top_dir, ConflictPolicy::Rename)?;
//! # Ok::<(), archive::ArchiveError>(())
//! ```

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Serialize;
// ===== 安全阈值常量 =====

/// 单个归档文件允许的最大条目数（防御 zip-bomb / 7z-bomb）。
///
/// 单元是 ZIP / 7z 内部的 "file entry"（含子目录占位），不是磁盘字节。
/// 超出时 [`ArchiveError::TooManyEntries`] 由 [`check_entry_safety`] 抛出。
pub const MAX_ENTRY_COUNT: usize = 1000;

/// 单条目压缩比上限（解压后字节 / 压缩前字节）。
///
/// 经验值 100 足以应对正常 deflate 条目，同时击退 42.zip / 35.zip 类递归炸弹。
/// 压缩大小为 0 时跳过此检查（避免异常元数据误报）。
pub const MAX_COMPRESSION_RATIO: u64 = 100;

// 文件名最大字节数；超出由 [`sanitize_entry_name`] 拒绝。
/// 归档格式枚举，决定走 `zip` crate 还是 `sevenz_rust2` 的解/压路径。
///
/// 通过 `serde(rename_all = "lowercase")` 序列化为小写字符串，便于前后端约定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    /// ZIP 格式，使用 `zip` crate 解压与压缩；ZIP 分支实现见 `compress_zip`。
    Zip,
    /// 7z 格式，使用 `sevenz_rust2` 解压与压缩；7z 分支实现见 `compress_sevenz`。
    SevenZ,
}

impl ArchiveFormat {
    /// 返回小写字符串字面量 `"zip"` 或 `"7z"`，用于：
    /// - 与前端约定的格式名（前端 store / UI 文案）
    /// - 文件名后缀构造（`role_xxx.zip` / `role_xxx.7z`）
    /// - serde 兼容序列化（虽然已有 `rename_all`，但直接 `as_str` 更可控）
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::SevenZ => "7z",
        }
    }
}

/// 导入时的目标目录冲突处理策略，由前端在导入对话框中选择。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConflictPolicy {
    /// 目标已存在：跳过本次导入，返回 [`ArchiveError::AlreadyExists`]，
    /// 由调用方（通常是前端）决定后续动作。
    Skip,
    /// 目标已存在：依次追加 `_2` ... `_999` 后缀；999 次都占用则用 Unix 毫秒时间戳兜底。
    Rename,
    /// 目标已存在：直接复用目录，原有内容保留（调用方负责清理）。
    Overwrite,
}

/// 归档操作统一错误类型。
///
/// 解压、压缩、路径解析、冲突处理、取消等失败都收敛到这里，便于上层 `From` 与前端展示。
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// 路径不是 zip / 7z 之一（魔数不匹配或文件过小）。
    #[error("不支持的压缩包格式: {0}")]
    UnsupportedFormat(String),
    /// 检测到路径遍历企图（条目名含 `..`、绝对路径、UNC）。
    #[error("路径遍历攻击: {0}")]
    PathTraversal(String),
    /// 文件名本身非法：空、过长、macOS metadata 等。
    /// 对 macOS metadata 调用方可作 warning 跳过，对其余情况应终止整个压缩包。
    #[error("非法文件名: {0}")]
    InvalidName(String),
    /// 条目数 ≥ [`MAX_ENTRY_COUNT`]。
    #[error("entry 数量超限: {0} 个, 限制 {MAX_ENTRY_COUNT}")]
    TooManyEntries(usize),
    /// 单条目解压后/压缩前比例 > [`MAX_COMPRESSION_RATIO`]。
    /// 字段 `actual` 为解压后字节、`compressed` 为压缩字节，供 UI 展示。
    #[error("压缩比超限 (解压/压缩 > {MAX_COMPRESSION_RATIO}): 解压 {actual} 字节, 压缩 {compressed} 字节")]
    CompressionRatio { actual: u64, compressed: u64 },
    /// `zip` crate 上游错误（解压损坏、不支持的方法等）。
    #[error("zip 错误: {0}")]
    Zip(String),
    /// `sevenz_rust2` crate 上游错误。
    #[error("7z 错误: {0}")]
    SevenZ(String),
    /// 标准 IO 错误透传（`#[from]`）。
    #[error("IO 错误: {0}")]
    Io(#[from] io::Error),
    /// 加密压缩包：当前不支持。需用户在本地解密后重新打包。
    #[error("密码保护的压缩包暂不支持")]
    PasswordProtected,
    /// 取消令牌 [`tokio_util::sync::CancellationToken`] 触发。
    /// 调用方需清理已写入的 staging 目录。
    #[error("操作被取消")]
    Cancelled,
    /// [`ConflictPolicy::Skip`] 命中：目标目录已存在。
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

/// 单条目进度事件。
///
/// 由 [`extract_zip`] / [`extract_sevenz`] / [`compress`] 通过 `on_entry` 回调推送。
/// 前端进度条 toast 订阅此类事件以更新文案与百分比。
#[derive(Debug, Clone, Default, Serialize)]
pub struct EntryEvent {
    /// 生命周期阶段。固定取值：
    /// - `"started"`：流程开始（仅有 `total` 字段有意义）
    /// - `"entry"`：单条目已写入
    /// - `"finished"`：流程完成
    /// - `"error"`：错误（一般通过 `Err(ArchiveError)` 传递，事件流少用）
    #[serde(rename = "phase")]
    pub phase: &'static str,
    /// 当前条目序号（1-based；"finished" 时 = total）
    pub index: usize,
    /// 总条目数
    pub total: usize,
    /// 当前条目原始名（未经过 [`sanitize_entry_name`]）
    pub name: String,
    /// 累计已写入字节
    pub bytes_done: u64,
    /// 压缩包声明的总字节（仅 [`extract_zip`] `"finished"` 事件有值）
    pub bytes_total: u64,
    /// 当前条目字节数
    pub bytes_entry: u64,
}

/// 一次解压流程的统计结果。
#[derive(Debug, Default, Clone, Serialize)]
pub struct ExtractSummary {
    /// 实际写入磁盘的字节数
    pub bytes_extracted: u64,
    /// 解压出的非目录条目数
    pub files_extracted: usize,
    /// 因 macOS metadata 被跳过的条目数（无害警告）
    pub skipped_macos_metadata: usize,
    /// 软警告列表（路径清洗跳过的详情等）
    pub warnings: Vec<String>,
}

/// [`resolve_target`] 的解析结果。
#[derive(Debug)]
pub struct TargetResolution {
    /// 最终写入目标的全路径
    pub target: PathBuf,
    /// 实际使用的目录名（[`ConflictPolicy::Rename`] 下可能与 `preferred` 不同）
    pub final_name: String,
    /// 解析动作，取值 `"created"` / `"renamed"` / `"overwritten"` 之一
    pub action: &'static str,
}
// ===== 1. 文件头魔数检测 =====

/// ZIP 归档头魔数：4 字节 `PK\x03\x04`。ZIP 空归档使用 `ZIP_EMPTY_MAGIC`（私有）。
pub const ZIP_MAGIC: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

/// 7z 归档头魔数：6 字节 `7z\xBC\xAF\x27\x1C`。
pub const SEVENZ_MAGIC: [u8; 6] = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];

// ZIP 空归档专用魔数（zip crate 内部对中央目录不存在的情况单独处理）。
const ZIP_EMPTY_MAGIC: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];

/// 通过读取文件前 6 个字节判断归档格式（zip / 7z）。
///
/// 仅依赖 magic，不解析整个压缩包，速度快，适合在 UI 文件选择后第一时间反馈。
///
/// # 参数
/// - `path`：磁盘上的归档文件路径
///
/// # 返回
/// 成功返回 `ArchiveFormat`；文件过小或魔数不匹配返回 [`ArchiveError::UnsupportedFormat`]；
/// 读取失败返回 [`ArchiveError::Io`]。
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

// ===== 子模块 =====

mod compress;
mod extract;
mod resolve;
mod safety;

pub use compress::compress;
pub use extract::{extract_zip, extract_sevenz};
pub use resolve::resolve_target;
pub use safety::{check_entry_safety, sanitize_entry_name, safe_join};

pub(super) fn map_sevenz_err(e: sevenz_rust2::Error) -> ArchiveError {
    match e {
        sevenz_rust2::Error::PasswordRequired => ArchiveError::PasswordProtected,
        other => ArchiveError::SevenZ(other.to_string()),
    }
}