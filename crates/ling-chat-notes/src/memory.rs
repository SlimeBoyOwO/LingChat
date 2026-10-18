use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::Note;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Raw,
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
        let norm = self
            .values
            .iter()
            .map(|value| value * value)
            .sum::<f32>()
            .sqrt();
        if self.chunk_version != chunk.version
            || self.model.dimension == 0
            || self.model.model.is_empty()
            || self.model.preprocessing.is_empty()
            || self.values.len() != self.model.dimension
            || self.values.iter().any(|value| !value.is_finite())
            || !norm.is_finite()
            || (norm - 1.0).abs() > 1e-3
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

pub fn memory_version(value: &impl Serialize) -> Result<String, serde_json::Error> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

impl MemoryDocument {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn note() -> Note {
        serde_json::from_str(r#"{"id":"legacy","content":"猫叫团团","created_at":"2026-10-01"}"#)
            .unwrap()
    }

    #[test]
    fn legacy_note_roundtrips_without_changing_storage_schema() {
        let note = note();
        let document = MemoryDocument::from_note(2, &note).unwrap();
        assert!(document.tags.is_empty());
        assert_eq!(document.scope, MemoryScope::Role);
        assert_eq!(document.event_status, EventStatus::Unknown);
        let json = serde_json::to_value(&note).unwrap();
        assert_eq!(json.as_object().unwrap().len(), 4);
        assert!(json.get("version").is_none());
        let encoded = serde_json::to_string(&document).unwrap();
        assert_eq!(
            serde_json::from_str::<MemoryDocument>(&encoded).unwrap(),
            document
        );
    }

    #[test]
    fn note_versions_track_content_tags_date_and_owner() {
        let mut note = note();
        let original = MemoryDocument::from_note(2, &note).unwrap();
        assert_eq!(original, MemoryDocument::from_note(2, &note).unwrap());
        assert_ne!(
            original.version,
            MemoryDocument::from_note(3, &note).unwrap().version
        );
        note.content.push('。');
        let edited = MemoryDocument::from_note(2, &note).unwrap();
        assert_ne!(original.version, edited.version);
        note.tags.push("宠物".into());
        let tagged = MemoryDocument::from_note(2, &note).unwrap();
        assert_ne!(edited.version, tagged.version);
        note.created_at.push_str("T12:00:00Z");
        assert_ne!(
            tagged.version,
            MemoryDocument::from_note(2, &note).unwrap().version
        );
    }

    #[test]
    fn documents_reject_mixed_scope_and_missing_or_duplicate_sources() {
        let mut document = MemoryDocument::from_note(2, &note()).unwrap();
        document.scope = MemoryScope::Save { save_id: 101 };
        assert!(document.validate().is_err());
        document.kind = MemoryKind::Diary;
        assert!(document.validate().is_err());
        document.sources = vec![MemorySource::Line {
            line_id: 1,
            version: "v1".into(),
        }];
        document.version = document.content_version().unwrap();
        document.validate().unwrap();
        document.sources.push(document.sources[0].clone());
        assert!(document.validate().is_err());
        document.sources.clear();
        assert!(document.validate().is_err());
    }

    #[test]
    fn chunks_reject_foreign_documents_and_stale_source_versions() {
        let document = MemoryDocument::from_note(2, &note()).unwrap();
        let mut chunk = MemoryChunk {
            id: "chunk-1".into(),
            document_id: document.id.clone(),
            ordinal: 0,
            content: document.content.clone(),
            version: "chunk-v1".into(),
            sources: document.sources.clone(),
        };
        chunk.version = chunk.content_version().unwrap();
        chunk.validate_for(&document).unwrap();
        chunk.document_id = "other".into();
        assert!(chunk.validate_for(&document).is_err());
        chunk.document_id = document.id.clone();
        chunk.sources = vec![MemorySource::Note {
            note_id: "legacy".into(),
            version: "stale".into(),
        }];
        assert!(chunk.validate_for(&document).is_err());
    }

    #[test]
    fn metadata_and_text_changes_require_new_document_and_chunk_versions() {
        let mut document = MemoryDocument::from_note(2, &note()).unwrap();
        let mut chunk = MemoryChunk {
            id: "chunk-1".into(),
            document_id: document.id.clone(),
            ordinal: 0,
            content: document.content.clone(),
            version: String::new(),
            sources: document.sources.clone(),
        };
        chunk.version = chunk.content_version().unwrap();
        chunk.content.push('。');
        assert!(chunk.validate_for(&document).is_err());
        document.event_status = EventStatus::Planned;
        assert!(document.validate().is_err());
        document.version = document.content_version().unwrap();
        document.validate().unwrap();
    }

    #[test]
    fn embeddings_reject_wrong_dimensions_nonfinite_values_and_stale_chunks() {
        let document = MemoryDocument::from_note(2, &note()).unwrap();
        let chunk = crate::chunking::whole_document_chunk(&document).unwrap();
        let mut record = MemoryEmbeddingRecord {
            model: EmbeddingVersion {
                model: "test".into(),
                dimension: 2,
                preprocessing: "v1".into(),
            },
            chunk_version: chunk.version.clone(),
            values: vec![1.0, 0.0],
        };
        record.validate_for(&chunk).unwrap();
        for values in [
            vec![1.0],
            vec![0.0, 0.0],
            vec![f32::NAN, 0.0],
            vec![f32::INFINITY, 0.0],
            vec![2.0, 0.0],
        ] {
            record.values = values;
            assert!(record.validate_for(&chunk).is_err());
        }
        record.values = vec![1.0, 0.0];
        record.chunk_version = "old".into();
        assert!(record.validate_for(&chunk).is_err());
    }
}
