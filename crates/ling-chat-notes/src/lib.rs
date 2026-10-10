//! LingChat 手动笔记存储库。
//!
//! 存储布局：`<data_dir>/game_data/notes/<角色名>.json`，每个角色一个文件，
//! 内容为 `Note` 数组。本 crate 不依赖 tauri / 数据库，只做文件层读写，
//! 角色名到文件的映射与路径安全由本 crate 负责。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// 一条手动记忆笔记。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: String,
}

/// 笔记库读写失败。
#[derive(Debug, Error)]
pub enum NoteError {
    #[error("读取角色 {role} 的笔记失败: {source}")]
    Read {
        role: String,
        #[source]
        source: io::Error,
    },
    #[error("解析角色 {role} 的笔记失败: {source}")]
    Parse {
        role: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("序列化笔记失败: {source}")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("保存笔记失败: {source}")]
    Write {
        #[source]
        source: io::Error,
    },
    #[error("笔记 {0} 不存在")]
    NotFound(String),
}

/// 按角色独立存储的笔记库。持有笔记根目录，无宿主依赖。
#[derive(Clone, Debug)]
pub struct NotesStore {
    root: PathBuf,
}

impl NotesStore {
    /// 以数据目录为基准创建存储，笔记落在 `<data_dir>/game_data/notes`。
    pub fn new(data_dir: impl AsRef<Path>) -> Self {
        Self {
            root: data_dir.as_ref().join("game_data").join("notes"),
        }
    }

    /// 角色笔记文件路径，文件名取自角色权威展示名，sanitize 后拼接。
    fn role_path(&self, role_name: &str) -> PathBuf {
        self.root
            .join(format!("{}.json", sanitize_role_name(role_name)))
    }

    /// 读取角色笔记；文件不存在时返回空列表。
    pub fn load(&self, role_name: &str) -> Result<Vec<Note>, NoteError> {
        let path = self.role_path(role_name);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&path).map_err(|source| NoteError::Read {
            role: role_name.to_string(),
            source,
        })?;
        serde_json::from_str(&content).map_err(|source| NoteError::Parse {
            role: role_name.to_string(),
            source,
        })
    }

    /// 原子写入角色笔记（.tmp + rename）。
    pub fn save(&self, role_name: &str, notes: &[Note]) -> Result<(), NoteError> {
        let content = serde_json::to_string_pretty(notes)
            .map_err(|source| NoteError::Serialize { source })?;
        atomic_write(&self.role_path(role_name), content.as_bytes())
            .map_err(|source| NoteError::Write { source })
    }

    /// 追加一条笔记，返回新笔记 id。
    pub fn add(
        &self,
        role_name: &str,
        content: String,
        tags: Vec<String>,
    ) -> Result<String, NoteError> {
        let mut notes = self.load(role_name)?;
        let note = Note {
            id: uuid::Uuid::new_v4().to_string(),
            content,
            tags,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let id = note.id.clone();
        notes.push(note);
        self.save(role_name, &notes)?;
        Ok(id)
    }

    /// 按 id 更新笔记的内容或标签；两项均为 `None` 时仅重写文件。
    pub fn update(
        &self,
        role_name: &str,
        id: &str,
        content: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<(), NoteError> {
        let mut notes = self.load(role_name)?;
        let Some(note) = notes.iter_mut().find(|note| note.id == id) else {
            return Err(NoteError::NotFound(id.to_string()));
        };
        if let Some(content) = content {
            note.content = content;
        }
        if let Some(tags) = tags {
            note.tags = tags;
        }
        self.save(role_name, &notes)
    }

    /// 按 id 删除笔记。
    pub fn delete(&self, role_name: &str, id: &str) -> Result<(), NoteError> {
        let mut notes = self.load(role_name)?;
        let before = notes.len();
        notes.retain(|note| note.id != id);
        if notes.len() == before {
            return Err(NoteError::NotFound(id.to_string()));
        }
        self.save(role_name, &notes)
    }
}

/// 清理角色名中的非法文件名字符，兜底防空/防 `..`。
pub fn sanitize_role_name(name: &str) -> String {
    let cleaned: String = name
        .trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-' || *c == ' ')
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "unknown".to_string()
    } else {
        cleaned
    }
}

/// 写同目录临时文件后原子替换目标，Windows 上也能覆盖已有文件。
fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("目标路径没有父目录: {}", path.display()),
        )
    })?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(content)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crud_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotesStore::new(dir.path());

        assert!(store.load("爱丽丝").unwrap().is_empty());

        let id = store
            .add("爱丽丝", "喜欢猫".into(), vec!["偏好".into()])
            .unwrap();
        let notes = store.load("爱丽丝").unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].content, "喜欢猫");
        assert_eq!(notes[0].tags, vec!["偏好".to_string()]);

        store
            .update("爱丽丝", &id, Some("其实喜欢狗".into()), None)
            .unwrap();
        assert_eq!(store.load("爱丽丝").unwrap()[0].content, "其实喜欢狗");

        store.delete("爱丽丝", &id).unwrap();
        assert!(store.load("爱丽丝").unwrap().is_empty());
    }

    #[test]
    fn missing_id_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotesStore::new(dir.path());
        assert!(matches!(
            store.delete("爱丽丝", "nope"),
            Err(NoteError::NotFound(_))
        ));
        assert!(matches!(
            store.update("爱丽丝", "nope", Some("x".into()), None),
            Err(NoteError::NotFound(_))
        ));
    }

    #[test]
    fn role_name_is_sanitized() {
        assert_eq!(sanitize_role_name("../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_role_name("   "), "unknown");
        assert_eq!(sanitize_role_name("爱丽丝"), "爱丽丝");
    }
}
