//! 身份卡的磁盘存储。
//!
//! 放在**独立目录** `<data_dir>/game_data/my_identities/`，而不是复用 `characters/`：
//! 启动时的角色同步逻辑会扫描 `characters/` 并按「AI 类型 + 文件夹名」登记角色，
//! 身份卡放进去会被误登记成第二个角色。独立目录同时带来完美的前向兼容——
//! 旧版本程序只扫 `characters/`，根本不认识这里。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use super::PlayerIdentity;

const IDENTITY_FILE: &str = "identity.yml";
const CURRENT_FILE: &str = "_current.json";
const TRASH_DIR: &str = ".trash";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct CurrentPointer {
    #[serde(default)]
    identity_id: String,
}

pub struct IdentityStore {
    root: PathBuf,
}

impl IdentityStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("game_data").join("my_identities"),
        }
    }

    /// 身份卡目录（供「打开文件夹」之类的命令使用）。
    pub fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("创建身份目录失败: {:?}", self.root))
    }

    // ---------- 读 ----------

    /// 扫描全部身份卡。解析失败的单张卡会被跳过并打日志，不影响其他卡。
    pub fn list(&self) -> Vec<PlayerIdentity> {
        self.scan().into_iter().map(|(_, identity)| identity).collect()
    }

    pub fn find_by_id(&self, id: &str) -> Option<PlayerIdentity> {
        let id = id.trim();
        if id.is_empty() {
            return None;
        }
        self.scan()
            .into_iter()
            .map(|(_, identity)| identity)
            .find(|identity| identity.id == id)
    }

    /// 当前使用的身份 id（全局）。未设置过返回 `None`。
    pub fn current_id(&self) -> Option<String> {
        let raw = fs::read_to_string(self.root.join(CURRENT_FILE)).ok()?;
        let pointer: CurrentPointer = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("解析 {:?} 失败: {}", self.root.join(CURRENT_FILE), e);
                return None;
            },
        };
        let id = pointer.identity_id.trim().to_string();
        if id.is_empty() {
            None
        } else {
            Some(id)
        }
    }

    pub fn current(&self) -> Option<PlayerIdentity> {
        self.current_id().and_then(|id| self.find_by_id(&id))
    }

    pub fn set_current_id(&self, id: &str) -> Result<()> {
        self.ensure_root()?;
        let pointer = CurrentPointer {
            identity_id: id.trim().to_string(),
        };
        let raw = serde_json::to_string_pretty(&pointer)?;
        fs::write(self.root.join(CURRENT_FILE), raw)
            .with_context(|| format!("写入当前身份失败: {:?}", self.root.join(CURRENT_FILE)))?;
        Ok(())
    }

    /// 读取「当前身份」；从未设置过时，**按老规则合成一张并立即落盘**。
    ///
    /// 这一步是必须的，不是可选优化：改造前「我」的名字是从当前 AI 角色卡上的
    /// `user_name` 现推的，切换 AI 角色时名字会跟着变。只有把合成结果落盘并记为当前身份，
    /// 老用户的「我」的名字才会稳定下来——否则这个改造对老用户体验为零变化。
    ///
    /// 落盘失败时返回内存里的合成卡（`synthetic = true`），功能仍可用，只是不持久。
    pub fn current_or_synthesize(&self, fallback_name: &str, fallback_subtitle: &str) -> PlayerIdentity {
        if let Some(existing) = self.current() {
            return existing;
        }

        let name = fallback_name.trim();
        let synthetic = PlayerIdentity {
            id: String::new(),
            name: if name.is_empty() { "用户".to_string() } else { name.to_string() },
            subtitle: fallback_subtitle.trim().to_string(),
            prompt: String::new(),
            avatar: None,
            relations: HashMap::new(),
            synthetic: true,
        };

        match self.save(&synthetic) {
            Ok(saved) => {
                if let Err(e) = self.set_current_id(&saved.id) {
                    tracing::warn!("记录当前身份失败: {e}");
                }
                tracing::info!("已按角色卡合成默认身份并落盘: id={} name={}", saved.id, saved.name);
                saved
            },
            Err(e) => {
                tracing::warn!("合成默认身份落盘失败，本次仅在内存中使用: {e}");
                synthetic
            },
        }
    }

    // ---------- 写 ----------

    /// 新建或更新一张身份卡。
    ///
    /// - `id` 为空 → 视为新建，生成不可变 id，并按名字分配文件夹；
    /// - `id` 已存在 → 沿用原文件夹（改名字不会搬家，关系与存档引用都不会断）；
    /// - `id` 非空但找不到 → 按该 id 新建（例如手工放进来的卡）。
    pub fn save(&self, identity: &PlayerIdentity) -> Result<PlayerIdentity> {
        self.ensure_root()?;

        let mut card = identity.clone();
        card.synthetic = false;
        card.name = card.name.trim().to_string();
        card.subtitle = card.subtitle.trim().to_string();

        let dir = match self.find_dir_by_id(&card.id) {
            Some(dir) => dir,
            None => {
                if card.id.trim().is_empty() {
                    card.id = new_identity_id();
                }
                self.alloc_dir(&card.name, &card.id)?
            },
        };

        fs::create_dir_all(&dir).with_context(|| format!("创建身份目录失败: {:?}", dir))?;
        let yaml = serde_yaml::to_string(&card).context("序列化身份卡失败")?;
        fs::write(dir.join(IDENTITY_FILE), yaml)
            .with_context(|| format!("写入身份卡失败: {:?}", dir.join(IDENTITY_FILE)))?;

        Ok(card)
    }

    /// 软删除：把卡片目录移到 `.trash/` 下。
    ///
    /// 调用方需要**先确认没有存档引用它**（`save_identity.identity_id`），
    /// 否则老存档会解析不到身份、回退成合成默认卡，用户会以为身份丢了。
    pub fn delete(&self, id: &str) -> Result<bool> {
        let id = id.trim();
        if id.is_empty() {
            return Ok(false);
        }
        let Some(dir) = self.find_dir_by_id(id) else {
            return Ok(false);
        };

        let trash = self.root.join(TRASH_DIR);
        fs::create_dir_all(&trash)?;
        let folder_name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| id.to_string());
        let target = trash.join(format!("{folder_name}-{}", now_millis()));

        fs::rename(&dir, &target)
            .with_context(|| format!("移入回收站失败: {:?} -> {:?}", dir, target))?;

        // 删掉的正好是当前身份 → 清空指针，下次会重新合成一张
        if self.current_id().as_deref() == Some(id) {
            let _ = fs::remove_file(self.root.join(CURRENT_FILE));
        }

        Ok(true)
    }

    // ---------- 内部 ----------

    /// 扫描出 `(目录, 身份卡)`，跳过隐藏目录（含 `.trash`）与无卡目录。
    fn scan(&self) -> Vec<(PathBuf, PlayerIdentity)> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };

        let mut out = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let Some(folder_name) = path.file_name().map(|s| s.to_string_lossy().to_string()) else {
                continue;
            };
            if folder_name.starts_with('.') {
                continue;
            }
            let file = path.join(IDENTITY_FILE);
            if !file.exists() {
                continue;
            }
            let raw = match fs::read_to_string(&file) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("读取身份卡失败 {:?}: {}", file, e);
                    continue;
                },
            };
            let mut card: PlayerIdentity = match serde_yaml::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("解析身份卡失败 {:?}: {}", file, e);
                    continue;
                },
            };
            // 兜底：文件里没写 id（手工放进来 / 老版本写的）就用文件夹名当 id，
            // 保证「每张卡都有稳定 id」这个不变量成立。
            if card.id.trim().is_empty() {
                card.id = folder_name.clone();
            }
            out.push((path, card));
        }

        out.sort_by(|a, b| a.1.name.cmp(&b.1.name));
        out
    }

    fn find_dir_by_id(&self, id: &str) -> Option<PathBuf> {
        let id = id.trim();
        if id.is_empty() {
            return None;
        }
        self.scan()
            .into_iter()
            .find(|(_, card)| card.id == id)
            .map(|(dir, _)| dir)
    }

    /// 按名字挑一个不冲突的文件夹名；重名时追加序号。
    fn alloc_dir(&self, name: &str, id: &str) -> Result<PathBuf> {
        let base = sanitize_folder(name);
        let base = if base.is_empty() { id.to_string() } else { base };

        let mut candidate = base.clone();
        let mut index = 2;
        loop {
            let dir = self.root.join(&candidate);
            if !dir.exists() {
                return Ok(dir);
            }
            candidate = format!("{base}-{index}");
            index += 1;
            if index > 1000 {
                anyhow::bail!("无法为身份分配目录名: {base}");
            }
        }
    }
}

/// 文件夹名净化：去掉路径分隔符与 Windows 保留字符，并裁剪空白与首尾点。
fn sanitize_folder(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    cleaned
        .trim()
        .trim_matches('.')
        .chars()
        .take(48)
        .collect::<String>()
        .trim()
        .to_string()
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 生成不可变身份 id。用毫秒时间戳即可：身份卡是低频创建的对象。
fn new_identity_id() -> String {
    format!("p{}", now_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_dangerous_chars() {
        assert_eq!(sanitize_folder("侦/探:林*默?"), "侦_探_林_默_");
        assert_eq!(sanitize_folder("  ..公主..  "), "公主");
        assert_eq!(sanitize_folder(""), "");
    }

    #[test]
    fn endpoint_folder_falls_back_to_id() {
        // 纯标点名字净化后为空 → 用 id 当文件夹名
        let store = IdentityStore::new(Path::new("/nonexistent"));
        assert!(sanitize_folder("///").is_empty());
        // 不实际写盘，只验证净化逻辑
        let _ = store;
    }
}
