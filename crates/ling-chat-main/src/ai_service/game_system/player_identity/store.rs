//! 身份卡的磁盘存储：`<data_dir>/game_data/player_identities/`。

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use super::PlayerIdentity;

const IDENTITY_FILE: &str = "identity.yml";
const CURRENT_FILE: &str = "_current.json";
const TRASH_DIR: &str = ".trash";
const IDENTITY_DIR: &str = "player_identities";

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

    /// 读取「当前身份」；从未设置过时按老规则合成一张并立即落盘。
    pub fn current_or_synthesize(
        &self,
        fallback_name: &str,
        fallback_subtitle: &str,
    ) -> PlayerIdentity {
        if let Some(existing) = self.current() {
            return existing;
        }

        let name = fallback_name.trim();
        let card = PlayerIdentity {
            id: String::new(),
            name: if name.is_empty() {
                "用户".to_string()
            } else {
                name.to_string()
            },
            subtitle: fallback_subtitle.trim().to_string(),
            prompt: String::new(),
            avatar: None,
            relations: HashMap::new(),
        };

        match self.save(&card) {
            Ok(saved) => {
                if let Err(e) = self.set_current_id(&saved.id) {
                    tracing::warn!("记录当前身份失败: {e}");
                }
                tracing::info!(
                    "已按角色卡合成默认身份并落盘: id={} name={}",
                    saved.id,
                    saved.name
                );
                saved
            },
            Err(e) => {
                tracing::warn!("合成默认身份落盘失败，本次仅在内存中使用: {e}");
                card
            },
        }
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

        Ok(true)
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
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn new_identity_id() -> String {
    format!("p{}", now_millis())
}
