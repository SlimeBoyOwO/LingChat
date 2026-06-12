//! 数据资源清单的数据结构。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 清单中单个文件的条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// SHA-256 哈希（hex 字符串）
    pub sha256: String,
    /// 文件大小（字节）
    pub size: u64,
}

/// 数据资源版本清单。
///
/// 由 `scripts/generate-data-manifest.js` 在构建时生成，
/// 随 GitHub Release 一同发布。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataManifest {
    /// 数据版本号（单调递增整数）
    pub data_version: u64,
    /// 所有默认资源文件（key = 相对 data/ 的路径，使用 `/` 分隔）
    pub files: HashMap<String, FileEntry>,
}

/// 返回给前端的数据更新概览。
///
/// 前端用此信息展示更新内容并让用户决定是否执行更新。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DataUpdateInfo {
    /// 是否有可用更新
    pub available: bool,
    /// 远程数据版本号
    pub new_version: u64,
    /// 本地数据版本号
    pub current_version: u64,
    /// 新增的文件路径列表
    pub files_to_add: Vec<String>,
    /// 修改的文件路径列表
    pub files_to_modify: Vec<String>,
    /// 移除的文件路径列表（仅限旧清单中存在但新清单中不存在的）
    pub files_to_remove: Vec<String>,
    /// 需要下载的总字节数（数据 zip 的预期大小）
    pub total_download_size: u64,
}

/// 数据更新执行结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataUpdateResult {
    /// 是否成功
    pub success: bool,
    /// 操作描述
    pub message: String,
    /// 新数据版本号
    pub new_version: u64,
}
