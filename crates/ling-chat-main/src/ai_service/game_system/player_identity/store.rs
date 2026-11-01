//! 身份卡的磁盘存储：`<data_dir>/game_data/player_identities/`。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::PlayerIdentity;

const IDENTITY_FILE: &str = "identity.yml";
const CURRENT_FILE: &str = "_current.json";
const PRIMARY_FILE: &str = "_primary_identity.json";
const TRASH_DIR: &str = ".trash";
const IDENTITY_DIR: &str = "player_identities";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct CurrentPointer {
    #[serde(default)]
    identity_id: String,
}

/// 主身份稳定标记：`_primary_identity.json`。
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct PrimaryIdentityMarker {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub primary_identity_id: String,
    #[serde(default)]
    pub migrated_at: String,
    #[serde(default)]
    pub mode: String,
}

pub struct IdentityStore {
    root: PathBuf,
}

impl IdentityStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            root: data_dir.join("game_data").join(IDENTITY_DIR),
        }
    }

    pub fn ensure_root(&self) -> Result<()> {
        fs::create_dir_all(&self.root).with_context(|| format!("创建身份目录失败: {:?}", self.root))
    }

    pub fn list(&self) -> Vec<PlayerIdentity> {
        self.scan()
            .into_iter()
            .map(|(_, identity)| identity)
            .collect()
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
        if id.is_empty() { None } else { Some(id) }
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

    /// 读取主身份标记；文件缺失或解析失败返回 None。
    pub fn primary_marker(&self) -> Option<PrimaryIdentityMarker> {
        let path = self.root.join(PRIMARY_FILE);
        let raw = fs::read_to_string(&path).ok()?;
        match serde_json::from_str(&raw) {
            Ok(marker) => Some(marker),
            Err(e) => {
                tracing::warn!("解析主身份标记失败 {:?}: {}", path, e);
                None
            },
        }
    }

    /// 主身份：标记文件指向的卡优先；标记丢失时扫 `is_primary: true` 兜底。
    pub fn primary_identity(&self) -> Option<PlayerIdentity> {
        if let Some(marker) = self.primary_marker() {
            if let Some(mut card) = self.find_by_id(&marker.primary_identity_id) {
                card.is_primary = true;
                return Some(card);
            }
        }
        self.flagged_primary()
    }

    /// 扫描带 `is_primary: true` 的身份卡（标记文件丢失时的兜底）。
    pub fn flagged_primary(&self) -> Option<PlayerIdentity> {
        self.scan()
            .into_iter()
            .map(|(_, identity)| identity)
            .find(|identity| identity.is_primary)
            .map(|mut card| {
                card.is_primary = true;
                card
            })
    }

    /// 把一张身份卡保存为主身份（`is_primary = true`）。
    pub fn set_primary(&self, identity: &PlayerIdentity) -> Result<PlayerIdentity> {
        let mut card = identity.clone();
        card.is_primary = true;
        self.save(&card)
    }

    /// 写主身份标记；迁移在最后一步调用，失败时下次启动靠 `is_primary` 兜底。
    pub fn write_primary_marker(&self, identity_id: &str, mode: &str) -> Result<()> {
        self.ensure_root()?;
        let marker = PrimaryIdentityMarker {
            version: 1,
            primary_identity_id: identity_id.trim().to_string(),
            migrated_at: now_rfc3339(),
            mode: mode.trim().to_string(),
        };
        let raw = serde_json::to_string_pretty(&marker)?;
        fs::write(self.root.join(PRIMARY_FILE), raw)
            .with_context(|| format!("写入主身份标记失败: {:?}", self.root.join(PRIMARY_FILE)))?;
        Ok(())
    }

    /// 删除主身份卡时同步清掉标记，避免下次启动读到悬空 id。
    pub fn clear_primary_marker(&self) {
        let _ = fs::remove_file(self.root.join(PRIMARY_FILE));
    }

    pub fn save(&self, identity: &PlayerIdentity) -> Result<PlayerIdentity> {
        self.ensure_root()?;

        let mut card = identity.clone();
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

    /// 软删除：把卡片目录移到 `.trash/` 下；调用方需先确认没有存档引用它。
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

        if self.current_id().as_deref() == Some(id) {
            let _ = fs::remove_file(self.root.join(CURRENT_FILE));
        }
        if self
            .primary_marker()
            .map(|marker| marker.primary_identity_id == id)
            .unwrap_or(false)
        {
            self.clear_primary_marker();
        }

        Ok(true)
    }

    /// 已删除身份（`.trash`）曾绑定的角色文件夹，避免下次启动把它们重新建回来。
    pub fn deleted_source_folders(&self) -> HashSet<String> {
        let trash = self.root.join(TRASH_DIR);
        let Ok(entries) = fs::read_dir(&trash) else {
            return HashSet::new();
        };
        let mut folders = HashSet::new();
        for entry in entries.flatten() {
            let path = entry.path().join(IDENTITY_FILE);
            let Ok(raw) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(card) = serde_yaml::from_str::<PlayerIdentity>(&raw) else {
                continue;
            };
            folders.extend(card.source_folders);
        }
        folders
    }

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
            let Some(folder_name) = path.file_name().map(|s| s.to_string_lossy().to_string())
            else {
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

    fn alloc_dir(&self, name: &str, id: &str) -> Result<PathBuf> {
        let base = sanitize_folder(name);
        let base = if base.is_empty() {
            id.to_string()
        } else {
            base
        };

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
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn new_identity_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    // 批量迁移可能在同毫秒连续建卡，追加进程内序号避免 id 撞车后互相覆盖目录。
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::Relaxed) % 10_000;
    format!("p{}{seq:04}", now_millis())
}
