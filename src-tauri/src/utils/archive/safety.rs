//! 解压安全防线：条目数 / 压缩比 / 路径清洗 / 路径拼接。
//!
//! 提供四个核心函数供 [`super::extract::extract_zip`] / [`super::extract::extract_sevenz`] 调用：
//! - [`check_entry_safety`] 单条目压缩比 + 数量闸门
//! - [`sanitize_entry_name`] 路径名清洗与危险名拒绝
//! - [`safe_join`] 目标子树二次路径逃逸校验
//! - [`is_macos_metadata`] macOS 元数据条目识别（私有 helper）

use std::path::{Path, PathBuf};

use super::{ArchiveError, MAX_COMPRESSION_RATIO, MAX_ENTRY_COUNT};

// 文件名最大字节数；超出由 [`sanitize_entry_name`] 拒绝。
const MAX_NAME_LEN: usize = 4096;

// ===== 2. 安全检查 =====

/// 单条目解压前的安全闸门。
///
/// 返回 `Ok` 表示条目可以解压，返回 `Err` 表示必须立即终止解压流程。
/// 由 [`super::extract::extract_zip`] / [`super::extract::extract_sevenz`] 在每条目写入磁盘前调用，是 zip-bomb 防御的关键路径。
///
/// # 参数
/// - `entry_index`：当前条目序号（0-based）
/// - `entry_compressed`：条目压缩后字节数（`size_compressed` from zip/7z）
/// - `entry_uncompressed`：条目解压后字节数（`size` from zip/7z）
///
/// # 返回
/// - `Ok(())`：通过检查
/// - `Err(TooManyEntries)`：序号 ≥ [`MAX_ENTRY_COUNT`]
/// - `Err(CompressionRatio{..})`：解压后/压缩前 > [`MAX_COMPRESSION_RATIO`]；压缩字节为 0 时跳过此检查
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
/// # 拒绝（返回 [`ArchiveError::InvalidName`] 或 [`ArchiveError::PathTraversal`]）
/// - 空名
/// - 长度 > 4 KiB
/// - macOS metadata：`__MACOSX/`、`._*`、`.DS_Store`
/// - 含 `..` 组件（在 `/` 或 `\` 切分后任一段为 `".."`）
/// - Unix 绝对路径（以 `/` 开头）
/// - Windows 盘符路径（如 `C:\` 或 `C:/`）
/// - UNC 路径（以 `\\` 开头）
///
/// # 替换（保留字符）
/// - Windows 保留字符 `\ : * ? " < > |` → `_`
/// - 控制字符（`is_control()`） → `_`
///
/// # 注意
/// 即使经过本函数清洗，仍需再经过 [`safe_join`] 进行目标根目录下的二次路径逃逸校验。
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

// 判断文件名是否属于 macOS 自动产生的 metadata（无需对外暴露）。
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

/// 在目标根目录下安全拼接条目路径。
///
/// 要求结果路径必须仍在 `dest_root` 子树内，否则 [`ArchiveError::PathTraversal`]。
/// 即使 `cleaned_name` 已经过 [`sanitize_entry_name`]，本函数作为最后防线仍需调用。
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
