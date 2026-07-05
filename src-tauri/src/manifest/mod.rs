//! 公共数据清单类型与工具函数。
//!
//! `DataManifest` 和 `FileEntry` 是 LingChat 中文件同步系统的基础类型，
//! 被以下模块共同使用：
//! - `resource_sync`: 从安装包资源同步默认文件到 data/ 工作目录
//! - `lan_sync`: 局域网全量数据同步
//! - `init::static_copy`: 移动端首次播种

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ─── 基础类型 ────────────────────────────────────────────────

/// 清单中单个文件的条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    /// SHA-256 哈希（hex 字符串）
    pub sha256: String,
    /// 文件大小（字节）
    pub size: u64,
    /// Unix 时间戳（秒），记录文件的最后修改时间。
    /// 用于 LAN 同步冲突解决。旧清单中无此字段时默认为 0。
    #[serde(default)]
    pub modified_at: i64,
}

/// 数据资源版本清单。
///
/// 由 `scripts/generate-data-manifest.js` 在构建时生成，
/// 记录某版本下所有默认资源文件的路径与校验信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataManifest {
    /// 数据版本号（单调递增整数）
    pub data_version: u64,
    /// 所有默认资源文件（key = 相对 data/ 的路径，使用 `/` 分隔）
    pub files: HashMap<String, FileEntry>,
}

/// 两个清单之间的差异。
#[derive(Debug, Clone)]
pub struct ManifestDiff {
    /// 新清单有而旧清单没有的文件
    pub files_to_add: Vec<String>,
    /// 两方都有但 SHA-256 或大小不同的文件
    pub files_to_modify: Vec<String>,
    /// 旧清单有而新清单没有的文件
    pub files_to_remove: Vec<String>,
}

// ─── 清单方法 ────────────────────────────────────────────────

impl DataManifest {
    /// 比较 self (旧/本地) 与 other (新/远程)，返回差异。
    ///
    /// 比较依据：先看 key 是否存在，再看 SHA-256 和 size。
    pub fn diff(&self, other: &DataManifest) -> ManifestDiff {
        let old_keys: HashSet<&String> = self.files.keys().collect();
        let new_keys: HashSet<&String> = other.files.keys().collect();

        let mut files_to_add: Vec<String> =
            new_keys.difference(&old_keys).map(|&k| k.clone()).collect();

        let mut files_to_modify: Vec<String> = old_keys
            .intersection(&new_keys)
            .filter(|&&k| {
                self.files[k].sha256 != other.files[k].sha256
                    || self.files[k].size != other.files[k].size
            })
            .map(|&k| k.clone())
            .collect();

        let files_to_remove: Vec<String> =
            old_keys.difference(&new_keys).map(|&k| k.clone()).collect();

        files_to_add.sort();
        files_to_modify.sort();

        ManifestDiff {
            files_to_add,
            files_to_modify,
            files_to_remove,
        }
    }

    /// 从文件加载清单。
    pub fn load(path: &Path) -> io::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        serde_json::from_str(&json)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// 将清单原子写入文件（.tmp → rename）。
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let tmp = path.with_extension(format!(
            "{}.tmp",
            path.extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default()
        ));
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        std::fs::write(&tmp, &json)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

// ─── 工具函数 ────────────────────────────────────────────────

/// 计算文件的 SHA-256 哈希（hex 字符串）。
pub fn compute_sha256(path: &Path) -> io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}
