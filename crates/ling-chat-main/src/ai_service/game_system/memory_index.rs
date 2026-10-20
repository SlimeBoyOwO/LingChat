//! 将可见历史与角色笔记同步到索引，只为新增或变化的分块计算向量。

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Result, ensure};
use ling_chat_notes::NotesStore;
use ling_chat_notes::chunking::{ChunkingError, chunk_document, whole_document_chunk};
use ling_chat_notes::memory::{
    EmbeddingVersion, EventStatus, IndexedMemoryChunk, IndexedMemoryDocument, MemoryDocument,
    MemoryEmbeddingRecord, MemoryKind, MemoryScope, MemoryScopeKey, MemorySource, memory_version,
};
use sea_orm::DatabaseConnection;

use crate::ai_service::embedding::EmbeddingEngine;
use crate::db::entities::line::LineAttribute;
use crate::db::managers::memory_index_repo::MemoryIndexRepo;

use super::memory_scope::{MemoryScopeSnapshot, ScopedMemoryLine};

/// 隔离模型调用，让索引与检索共用同一套版本、分词和嵌入规则。
pub trait MemoryEncoder: Send + Sync {
    fn version(&self) -> EmbeddingVersion;
    fn max_tokens(&self) -> usize;
    fn count_tokens(&self, text: &str) -> Result<usize>;
    fn embed(&self, text: &str) -> Result<Vec<f32>>;
}

impl MemoryEncoder for EmbeddingEngine {
    fn version(&self) -> EmbeddingVersion {
        self.memory_model_version()
    }
    fn max_tokens(&self) -> usize {
        self.max_sequence_length()
    }
    fn count_tokens(&self, text: &str) -> Result<usize> {
        self.token_count(text)
    }
    fn embed(&self, text: &str) -> Result<Vec<f32>> {
        EmbeddingEngine::embed(self, text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexMode {
    Incremental,
    Rebuild,
}

#[derive(Clone, Debug)]
pub struct MemoryIndexReport {
    pub scope_key: MemoryScopeKey,
    pub index_version: String,
    pub documents: usize,
    pub chunks: usize,
    pub changed_documents: usize,
    pub embedded: usize,
    pub reused: usize,
    pub lexical_only: usize,
    pub embedding_failures: usize,
    pub oversized_chunks: usize,
}

pub struct MemoryIndexer {
    encoder: Arc<dyn MemoryEncoder>,
    token_budget: usize,
}

impl MemoryIndexer {
    pub fn new(encoder: Arc<dyn MemoryEncoder>) -> Self {
        Self {
            token_budget: 480.min(encoder.max_tokens()),
            encoder,
        }
    }

    pub async fn sync(
        &self,
        db: &DatabaseConnection,
        store: &NotesStore,
        save_id: i32,
        role_id: i32,
        role_display_name: &str,
        mode: IndexMode,
    ) -> Result<MemoryIndexReport> {
        ensure!(self.token_budget > 2, "模型 token 上限不足");
        let scope =
            MemoryScopeSnapshot::load(db, store, save_id, role_id, role_display_name).await?;
        let encoder = self.encoder.clone();
        let model = tokio::task::spawn_blocking(move || encoder.version()).await?;
        let existing = MemoryIndexRepo::load(db, &scope, &model).await?;
        let generation = existing.generation;
        let old_key = existing.scope_key;
        let old_index_version = existing.index_version;
        let encoder = self.encoder.clone();
        let token_budget = self.token_budget;
        let working_scope = scope.clone();
        // 分词和 ONNX 推理是同步计算，放到阻塞线程，不占用异步执行线程。
        let (desired, changed, report) = tokio::task::spawn_blocking(move || {
            prepare_index(
                &working_scope,
                existing.documents,
                encoder.as_ref(),
                &model,
                token_budget,
                mode,
            )
        })
        .await??;
        // 计算期间发生编辑或撤回时，旧任务不能覆盖新历史的索引。
        let current =
            MemoryScopeSnapshot::load(db, store, save_id, role_id, role_display_name).await?;
        ensure!(
            current.key() == scope.key(),
            "历史或笔记已变化，丢弃旧索引任务"
        );
        if mode == IndexMode::Rebuild
            || !changed.is_empty()
            || old_key.as_ref() != Some(scope.key())
            || old_index_version.as_ref() != Some(&report.index_version)
        {
            MemoryIndexRepo::commit(
                db,
                &current,
                generation,
                &report.index_version,
                &desired,
                &changed,
            )
            .await?;
        }
        if report.embedding_failures > 0 || report.oversized_chunks > 0 {
            tracing::warn!(
                failures = report.embedding_failures,
                oversized = report.oversized_chunks,
                "部分记忆保留词法索引，本次未保存向量"
            );
        }
        Ok(report)
    }
}

fn prepare_index(
    scope: &MemoryScopeSnapshot,
    existing: Vec<IndexedMemoryDocument>,
    encoder: &dyn MemoryEncoder,
    model: &EmbeddingVersion,
    budget: usize,
    mode: IndexMode,
) -> Result<(
    Vec<IndexedMemoryDocument>,
    HashSet<String>,
    MemoryIndexReport,
)> {
    let mut documents = raw_documents(scope)?;
    documents.extend(
        scope
            .notes()
            .iter()
            .cloned()
            .map(|document| (document, false)),
    );
    documents.extend(
        existing
            .iter()
            .filter(|item| item.document.kind == MemoryKind::Diary)
            .map(|item| (item.document.clone(), false)),
    );
    let old: HashMap<_, _> = existing
        .into_iter()
        .map(|item| (item.document.id.clone(), item))
        .collect();
    let mut desired = Vec::new();
    let mut changed = HashSet::new();
    let mut report = MemoryIndexReport {
        scope_key: scope.key().clone(),
        index_version: String::new(),
        documents: 0,
        chunks: 0,
        changed_documents: 0,
        embedded: 0,
        reused: 0,
        lexical_only: 0,
        embedding_failures: 0,
        oversized_chunks: 0,
    };
    // 同一次同步只有普通分块和完整工具事务两种策略，提前计算版本供所有文档复用。
    let chunking_versions = [
        memory_version(&("token-chunking-v3", budget, false, model))?,
        memory_version(&("token-chunking-v3", budget, true, model))?,
    ];
    for (document, atomic) in documents {
        let chunking_version = chunking_versions[usize::from(atomic)].clone();
        let previous = old.get(&document.id);
        if mode == IndexMode::Incremental {
            // 内容、来源、分块策略和向量都完整时，跳过重复分词与推理。
            if let Some(previous) = previous.filter(|previous| {
                previous.document == document
                    && previous.chunking_version == chunking_version
                    && complete_cached_chunks(previous)
            }) {
                report.chunks += previous.chunks.len();
                report.reused += previous.chunks.len();
                desired.push(previous.clone());
                continue;
            }
        }
        let chunks = match chunk_document(&document, budget, atomic, |text| {
            encoder
                .count_tokens(text)
                .map_err(|error| error.to_string())
        }) {
            Ok(chunks) => chunks,
            // 模型不可用时保留整篇原文，关键词与标签检索仍能使用。
            Err(ChunkingError::Tokenizer(_)) => vec![whole_document_chunk(&document)?],
            Err(error) => return Err(error.into()),
        };
        let mut indexed = IndexedMemoryDocument {
            document,
            chunking_version,
            chunks: vec![],
        };
        for chunk in chunks {
            let cached = if mode == IndexMode::Incremental {
                previous
                    .filter(|item| item.chunking_version == indexed.chunking_version)
                    .and_then(|item| item.chunks.iter().find(|item| item.chunk == chunk))
                    .and_then(|item| item.embedding.clone())
            } else {
                None
            };
            let embedding = if let Some(cached) = cached {
                report.reused += 1;
                Some(cached)
            } else {
                match encoder.count_tokens(&chunk.content) {
                    Ok(tokens) if tokens <= budget => {
                        let record = encoder
                            .embed(&chunk.content)
                            .map(|values| MemoryEmbeddingRecord {
                                model: model.clone(),
                                chunk_version: chunk.version.clone(),
                                values,
                            })
                            .and_then(|record| {
                                record.validate_for(&chunk)?;
                                Ok(record)
                            });
                        match record {
                            Ok(record) => {
                                report.embedded += 1;
                                Some(record)
                            },
                            Err(_) => {
                                report.embedding_failures += 1;
                                None
                            },
                        }
                    },
                    Ok(_) => {
                        report.oversized_chunks += 1;
                        None
                    },
                    Err(_) => {
                        report.embedding_failures += 1;
                        None
                    },
                }
            };
            report.chunks += 1;
            if embedding.is_none() {
                report.lexical_only += 1;
            }
            indexed.chunks.push(IndexedMemoryChunk { chunk, embedding });
        }
        if previous != Some(&indexed) || mode == IndexMode::Rebuild {
            changed.insert(indexed.document.id.clone());
        }
        desired.push(indexed);
    }
    desired.sort_by(|left, right| left.document.id.cmp(&right.document.id));
    report.documents = desired.len();
    report.changed_documents = changed.len();
    let manifest: Vec<_> = desired
        .iter()
        .map(|item| {
            (
                &item.document.id,
                &item.document.version,
                &item.chunking_version,
                item.chunks
                    .iter()
                    .map(|item| (&item.chunk.version, item.embedding.is_some()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    report.index_version =
        memory_version(&("memory-index-v2:turn-sentence-v2", budget, model, manifest))?;
    Ok((desired, changed, report))
}

// 必须按顺序无遗漏地还原原文，缺块或缺向量时才会重新补齐。
fn complete_cached_chunks(indexed: &IndexedMemoryDocument) -> bool {
    if indexed.document.content.trim().is_empty() {
        return indexed.chunks.is_empty();
    }
    let mut offset = 0;
    for (ordinal, item) in indexed.chunks.iter().enumerate() {
        let chunk = &item.chunk;
        if chunk.ordinal as usize != ordinal
            || chunk.id != format!("{}:{ordinal}", indexed.document.id)
            || chunk.sources != indexed.document.sources
            || item.embedding.is_none()
            || !indexed.document.content[offset..].starts_with(&chunk.content)
        {
            return false;
        }
        offset += chunk.content.len();
    }
    offset == indexed.document.content.len()
}

fn raw_documents(scope: &MemoryScopeSnapshot) -> Result<Vec<(MemoryDocument, bool)>> {
    let mut groups: Vec<Vec<&ScopedMemoryLine>> = Vec::new();
    let mut has_assistant = false;
    // 以用户发言开启新一轮，避免每条短台词都成为零散的文档。
    for line in scope.lines() {
        if groups.is_empty() || (line.attribute == LineAttribute::User && has_assistant) {
            groups.push(vec![]);
            has_assistant = false;
        }
        has_assistant |= line.attribute == LineAttribute::Assistant;
        groups.last_mut().unwrap().push(line);
    }
    groups
        .into_iter()
        .map(|group| {
            let MemorySource::Line { line_id, .. } = group[0].source else {
                unreachable!()
            };
            let content = group
                .iter()
                .map(|line| {
                    let speaker =
                        line.display_name
                            .clone()
                            .unwrap_or_else(|| match line.sender_role_id {
                                Some(id) => format!("发言者#{id}"),
                                None => "场景／系统事件".into(),
                            });
                    let label = match line.attribute {
                        LineAttribute::User => "用户或场景提供的记录",
                        LineAttribute::Assistant => "角色发言",
                        LineAttribute::Tool => "工具返回数据",
                        LineAttribute::System => "系统记录",
                    };
                    let mut text = format!("{label}（{speaker}）：{}", line.content);
                    if let Some(action) = &line.action_content {
                        text.push_str(&format!("（动作：{action}）"));
                    }
                    if let Some(call) = &line.tool_call {
                        text.push_str(&format!("\n工具调用请求：{call}"));
                    }
                    text
                })
                .collect::<Vec<_>>()
                .join("\n");
            // 带工具事务的回合整体保留，不能把请求和返回结果拆成孤立记忆。
            let atomic = group.iter().any(|line| line.transaction_sources.len() > 1);
            let mut document = MemoryDocument {
                id: format!(
                    "raw:{}:{}:{line_id}",
                    scope.key().save_id,
                    scope.key().role_id
                ),
                kind: MemoryKind::Raw,
                role_id: scope.key().role_id,
                scope: MemoryScope::Save {
                    save_id: scope.key().save_id,
                },
                content,
                tags: vec![],
                recorded_at: None,
                event_time: None,
                event_status: EventStatus::Unknown,
                version: String::new(),
                sources: group.iter().map(|line| line.source.clone()).collect(),
            };
            document.version = document.content_version()?;
            ensure!(scope.allows_document(&document), "原文分组包含无效来源");
            Ok((document, atomic))
        })
        .collect()
}
