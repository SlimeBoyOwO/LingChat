//! 记忆文档、分块和来源的共同约定；数据库访问与角色权限由宿主负责。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::Note;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Raw,
    // 预留日记类型；这里只描述数据，不负责生成日记。
    Diary,
    Note,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemoryScope {
    Save { save_id: i32 },
    Role,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MemorySource {
    Line { line_id: i32, version: String },
    Note { note_id: String, version: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Unknown,
    Happened,
    Planned,
    Hypothetical,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryDocument {
    pub id: String,
    pub kind: MemoryKind,
    pub role_id: i32,
    pub scope: MemoryScope,
    pub content: String,
    pub tags: Vec<String>,
    pub recorded_at: Option<String>,
    pub event_time: Option<String>,
    pub event_status: EventStatus,
    pub version: String,
    pub sources: Vec<MemorySource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryChunk {
    pub id: String,
    pub document_id: String,
    pub ordinal: u32,
    pub content: String,
    pub version: String,
    pub sources: Vec<MemorySource>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingVersion {
    pub model: String,
    pub dimension: usize,
    pub preprocessing: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MemoryScopeKey {
    pub save_id: i32,
    pub role_id: i32,
    pub history_version: String,
    pub notes_version: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemorySearchHit {
    pub document_id: String,
    pub document_version: String,
    pub kind: MemoryKind,
    pub recorded_at: Option<String>,
    pub event_time: Option<String>,
    pub event_status: EventStatus,
    pub chunk: MemoryChunk,
    pub score: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoryEmbeddingRecord {
    pub model: EmbeddingVersion,
    pub chunk_version: String,
    pub values: Vec<f32>,
}

impl MemoryEmbeddingRecord {
    pub fn validate_for(&self, chunk: &MemoryChunk) -> Result<(), MemoryContractError> {
        if self.chunk_version != chunk.version
            || self.model.model.is_empty()
            || self.model.preprocessing.is_empty()
            || !valid_memory_vector(&self.values, self.model.dimension)
        {
            return Err(MemoryContractError::Invalid(
                "向量必须匹配分块版本、模型维度且归一化",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IndexedMemoryChunk {
    pub chunk: MemoryChunk,
    pub embedding: Option<MemoryEmbeddingRecord>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IndexedMemoryDocument {
    pub document: MemoryDocument,
    #[serde(default)]
    pub chunking_version: String,
    pub chunks: Vec<IndexedMemoryChunk>,
}

#[derive(Debug, Error)]
pub enum MemoryContractError {
    #[error("无效的记忆契约: {0}")]
    Invalid(&'static str),
    #[error("记忆版本序列化失败: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// 查询、索引和缓存统一检查维度与单位长度，避免各处采用不同的校验规则。
pub fn valid_memory_vector(values: &[f32], dimension: usize) -> bool {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    dimension > 0
        && values.len() == dimension
        && values.iter().all(|value| value.is_finite())
        && norm.is_finite()
        && (norm - 1.0).abs() <= 1e-3
}

/// 内容和来源一起参与版本计算，编辑原文后旧索引与旧缓存便无法继续匹配。
pub fn memory_version(value: &impl Serialize) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

impl MemoryDocument {
    /// 在读取时转换旧笔记，不改变笔记文件原有的存储格式。
    pub fn from_note(role_id: i32, note: &Note) -> Result<Self, MemoryContractError> {
        let version = memory_version(&(
            "note-v1",
            role_id,
            &note.id,
            &note.content,
            &note.tags,
            &note.created_at,
        ))?;
        let mut document = Self {
            id: format!("note:{role_id}:{}", note.id),
            kind: MemoryKind::Note,
            role_id,
            scope: MemoryScope::Role,
            content: note.content.clone(),
            tags: note.tags.clone(),
            recorded_at: Some(note.created_at.clone()),
            event_time: None,
            event_status: EventStatus::Unknown,
            version: String::new(),
            sources: vec![MemorySource::Note {
                note_id: note.id.clone(),
                version,
            }],
        };
        document.version = document.content_version()?;
        document.validate()?;
        Ok(document)
    }

    pub fn content_version(&self) -> Result<String, serde_json::Error> {
        memory_version(&(
            "document-v1",
            &self.id,
            self.kind,
            self.role_id,
            &self.scope,
            &self.content,
            &self.tags,
            &self.recorded_at,
            &self.event_time,
            self.event_status,
            &self.sources,
        ))
    }

    pub fn validate(&self) -> Result<(), MemoryContractError> {
        use MemoryContractError::Invalid;
        if self.id.trim().is_empty() || self.version.trim().is_empty() || self.role_id <= 0 {
            return Err(Invalid("文档必须具有 ID、版本和有效角色"));
        }
        if self.sources.is_empty() {
            return Err(Invalid("文档必须保留来源"));
        }
        let mut identities = HashSet::new();
        for source in &self.sources {
            let identity = match source {
                MemorySource::Line { line_id, version } if *line_id > 0 && !version.is_empty() => {
                    format!("line:{line_id}")
                },
                MemorySource::Note { note_id, version }
                    if !note_id.trim().is_empty() && !version.is_empty() =>
                {
                    format!("note:{note_id}")
                },
                _ => return Err(Invalid("来源必须具有有效 ID 和版本")),
            };
            if !identities.insert(identity) {
                return Err(Invalid("同一文档不能重复引用同一来源"));
            }
        }
        // 原文跟随存档分支，手动笔记跟随角色，二者不能混用来源。
        match (&self.kind, &self.scope) {
            (MemoryKind::Note, MemoryScope::Role)
                if self.sources.len() == 1
                    && matches!(self.sources[0], MemorySource::Note { .. }) => {},
            (MemoryKind::Raw | MemoryKind::Diary, MemoryScope::Save { save_id })
                if *save_id > 0
                    && self
                        .sources
                        .iter()
                        .all(|source| matches!(source, MemorySource::Line { .. })) => {},
            _ => {
                return Err(Invalid(
                    "原文和日记属于存档，手动笔记属于角色，来源类型必须一致",
                ));
            },
        }
        if self.version != self.content_version()? {
            return Err(Invalid("文档内容和来源与版本不符"));
        }
        Ok(())
    }
}

impl MemoryChunk {
    pub fn content_version(&self) -> Result<String, serde_json::Error> {
        memory_version(&(
            "chunk-v1",
            &self.id,
            &self.document_id,
            self.ordinal,
            &self.content,
            &self.sources,
        ))
    }

    /// 分块必须属于当前文档，且只能引用该文档已经声明的来源。
    pub fn validate_for(&self, document: &MemoryDocument) -> Result<(), MemoryContractError> {
        document.validate()?;
        if self.id.trim().is_empty()
            || self.version.trim().is_empty()
            || self.document_id != document.id
            || self.sources.is_empty()
            || self.sources.iter().collect::<HashSet<_>>().len() != self.sources.len()
            || !self
                .sources
                .iter()
                .all(|source| document.sources.contains(source))
            || self.version != self.content_version()?
        {
            return Err(MemoryContractError::Invalid(
                "分块必须关联所属文档及其有效来源",
            ));
        }
        Ok(())
    }
}
