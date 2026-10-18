use std::sync::Arc;

use anyhow::{Result, ensure};
use ling_chat_notes::NotesStore;
use ling_chat_notes::memory::{
    EmbeddingVersion, IndexedMemoryDocument, MemoryScopeKey, MemorySearchHit, memory_version,
};
use ling_chat_notes::memory_retrieval::{
    ExplainedMemoryHit, HybridSearchResult, MatchExplanation, SearchDiagnostics, SearchQuery,
    SearchSettings, hybrid_search, render_reference,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::managers::memory_index_repo::{MemoryIndexRepo, MemoryIndexSnapshot};

use super::memory_cache::{CacheUsage, MemoryCacheConfig, MemoryCacheStats, MemoryCaches};
use super::memory_index::MemoryEncoder;
use super::memory_scope::MemoryScopeSnapshot;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryVectorStatus {
    Ready,
    EmptyQuery,
    NoIndexedVectors,
    TooManyTokens,
    TokenizerUnavailable,
    InferenceFailed,
    InvalidVector,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MemorySearchResponse {
    pub scope_key: MemoryScopeKey,
    pub index_version: String,
    pub index_is_current: bool,
    pub model: EmbeddingVersion,
    pub settings_version: String,
    pub query: SearchQuery,
    pub vector_status: QueryVectorStatus,
    pub result: HybridSearchResult,
    pub cache: CacheUsage,
}

pub struct MemorySearcher {
    encoder: Arc<dyn MemoryEncoder>,
    caches: Arc<MemoryCaches>,
}

#[cfg(test)]
#[path = "memory_search_real_tests.rs"]
mod real_tests;

impl MemorySearcher {
    pub fn new(encoder: Arc<dyn MemoryEncoder>) -> Self {
        Self::with_cache_config(encoder, MemoryCacheConfig::default())
    }

    pub fn with_cache_config(encoder: Arc<dyn MemoryEncoder>, config: MemoryCacheConfig) -> Self {
        Self {
            encoder,
            caches: Arc::new(MemoryCaches::new(config)),
        }
    }

    pub fn cache_stats(&self) -> MemoryCacheStats {
        self.caches.stats()
    }

    pub fn clear_caches(&self) {
        self.caches.clear();
    }

    pub async fn search(
        &self,
        db: &DatabaseConnection,
        store: &NotesStore,
        save_id: i32,
        role_id: i32,
        role_display_name: &str,
        query: &str,
        recent_context: &str,
        settings: &SearchSettings,
    ) -> Result<MemorySearchResponse> {
        let settings_version = settings.version()?;
        let query = SearchQuery::new(query, recent_context)?;
        let scope =
            MemoryScopeSnapshot::load(db, store, save_id, role_id, role_display_name).await?;
        let encoder = self.encoder.clone();
        let model = tokio::task::spawn_blocking(move || encoder.version()).await?;
        let snapshot = MemoryIndexRepo::load(db, &scope, &model).await?;
        let index_is_current = snapshot.scope_key.as_ref() == Some(scope.key());
        let working_query = query.clone();
        let working_model = model.clone();
        let working_key = scope.key().clone();
        let settings = settings.clone();
        let working_settings_version = settings_version.clone();
        let encoder = self.encoder.clone();
        let caches = self.caches.clone();
        let (result, vector_status, cache, index_version, result_key) =
            tokio::task::spawn_blocking(move || -> Result<_> {
                let index_version = index_fingerprint(&snapshot, &working_model)?;
                let result_key = memory_version(&(
                    "memory-result-cache-v1",
                    &working_key,
                    &index_version,
                    &working_model,
                    encoder.max_tokens(),
                    &working_query,
                    &working_settings_version,
                ))?;
                let mut cache = CacheUsage::default();
                if let Some(cached) = caches.get::<CachedResult>(false, &result_key) {
                    if let Some(result) = cached.restore(&snapshot.documents, &settings) {
                        cache.result_hit = true;
                        return Ok((
                            result,
                            cached.vector_status,
                            cache,
                            index_version,
                            result_key,
                        ));
                    }
                    caches.remove(false, &result_key);
                }
                let has_vectors = snapshot.documents.iter().any(|document| {
                    document
                        .chunks
                        .iter()
                        .any(|chunk| chunk.embedding.is_some())
                });
                let vector_key = memory_version(&(
                    "memory-query-vector-v1",
                    &working_model,
                    encoder.max_tokens(),
                    &working_query.effective,
                ))?;
                let (vector, mut status) = query_vector(
                    encoder.as_ref(),
                    &caches,
                    &vector_key,
                    &working_query,
                    &working_model,
                    has_vectors,
                    &mut cache,
                );
                caches.record_search();
                let result = hybrid_search(
                    &snapshot.documents,
                    &working_query,
                    &working_model,
                    vector.as_deref(),
                    &settings,
                )?;
                if result.diagnostics.invalid_query_vector {
                    status = QueryVectorStatus::InvalidVector;
                }
                Ok((result, status, cache, index_version, result_key))
            })
            .await??;
        let current =
            MemoryScopeSnapshot::load(db, store, save_id, role_id, role_display_name).await?;
        ensure!(
            current.key() == scope.key(),
            "历史或笔记已变化，丢弃旧检索结果"
        );
        if !cache.result_hit
            && matches!(
                vector_status,
                QueryVectorStatus::Ready
                    | QueryVectorStatus::NoIndexedVectors
                    | QueryVectorStatus::EmptyQuery
            )
        {
            self.caches.insert(
                false,
                result_key,
                &CachedResult::from_result(&result, vector_status.clone()),
            );
        }
        Ok(MemorySearchResponse {
            scope_key: scope.key().clone(),
            index_version,
            index_is_current,
            model,
            settings_version,
            query,
            vector_status,
            result,
            cache,
        })
    }
}

fn query_vector(
    encoder: &dyn MemoryEncoder,
    caches: &MemoryCaches,
    key: &str,
    query: &SearchQuery,
    model: &EmbeddingVersion,
    has_vectors: bool,
    usage: &mut CacheUsage,
) -> (Option<Vec<f32>>, QueryVectorStatus) {
    if query.original.is_empty() {
        return (None, QueryVectorStatus::EmptyQuery);
    }
    if !has_vectors {
        return (None, QueryVectorStatus::NoIndexedVectors);
    }
    if let Some(vector) = caches.get::<Vec<f32>>(true, key) {
        if valid_vector(&vector, model.dimension) {
            usage.query_vector_hit = true;
            return (Some(vector), QueryVectorStatus::Ready);
        }
        caches.remove(true, key);
    }
    match encoder.count_tokens(&query.effective) {
        Err(_) => (None, QueryVectorStatus::TokenizerUnavailable),
        Ok(count) if count > encoder.max_tokens() => (None, QueryVectorStatus::TooManyTokens),
        Ok(_) => match encoder.embed(&query.effective) {
            Ok(vector) => {
                if valid_vector(&vector, model.dimension) {
                    caches.insert(true, key.to_owned(), &vector);
                }
                (Some(vector), QueryVectorStatus::Ready)
            },
            Err(_) => (None, QueryVectorStatus::InferenceFailed),
        },
    }
}

fn valid_vector(values: &[f32], dimension: usize) -> bool {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    dimension > 0
        && values.len() == dimension
        && values.iter().all(|value| value.is_finite())
        && norm.is_finite()
        && (norm - 1.0).abs() <= 1e-3
}

fn index_fingerprint(snapshot: &MemoryIndexSnapshot, model: &EmbeddingVersion) -> Result<String> {
    let identity = memory_version(&(
        "search-index-v2",
        snapshot.generation,
        &snapshot.index_version,
        model,
        snapshot
            .documents
            .iter()
            .map(|document| {
                (
                    &document.document.id,
                    &document.document.version,
                    &document.chunking_version,
                    document
                        .chunks
                        .iter()
                        .map(|chunk| {
                            (
                                &chunk.chunk.id,
                                &chunk.chunk.version,
                                chunk.embedding.is_some(),
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>(),
    ))?;
    let mut digest = Sha256::new();
    digest.update(identity.as_bytes());
    for document in &snapshot.documents {
        for chunk in &document.chunks {
            if let Some(embedding) = &chunk.embedding {
                for value in &embedding.values {
                    digest.update(value.to_le_bytes());
                }
            }
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[derive(Serialize, Deserialize)]
struct CachedHit {
    document_id: String,
    document_version: String,
    chunk_id: String,
    chunk_version: String,
    score: f32,
    explanation: MatchExplanation,
}

#[derive(Serialize, Deserialize)]
struct CachedResult {
    hits: Vec<CachedHit>,
    diagnostics: SearchDiagnostics,
    vector_status: QueryVectorStatus,
}

impl CachedResult {
    fn from_result(result: &HybridSearchResult, vector_status: QueryVectorStatus) -> Self {
        Self {
            hits: result
                .hits
                .iter()
                .map(|item| CachedHit {
                    document_id: item.hit.document_id.clone(),
                    document_version: item.hit.document_version.clone(),
                    chunk_id: item.hit.chunk.id.clone(),
                    chunk_version: item.hit.chunk.version.clone(),
                    score: item.hit.score,
                    explanation: item.explanation.clone(),
                })
                .collect(),
            diagnostics: result.diagnostics.clone(),
            vector_status,
        }
    }

    fn restore(
        &self,
        documents: &[IndexedMemoryDocument],
        settings: &SearchSettings,
    ) -> Option<HybridSearchResult> {
        if self.hits.len() > settings.max_hits {
            return None;
        }
        let mut hits = Vec::new();
        let mut characters = 0;
        for cached in &self.hits {
            let indexed = documents.iter().find(|document| {
                document.document.id == cached.document_id
                    && document.document.version == cached.document_version
            })?;
            let chunk = indexed.chunks.iter().find(|chunk| {
                chunk.chunk.id == cached.chunk_id && chunk.chunk.version == cached.chunk_version
            })?;
            let document = &indexed.document;
            let hit = MemorySearchHit {
                document_id: document.id.clone(),
                document_version: document.version.clone(),
                kind: document.kind,
                recorded_at: document.recorded_at.clone(),
                event_time: document.event_time.clone(),
                event_status: document.event_status,
                chunk: chunk.chunk.clone(),
                score: cached.score,
            };
            let reference = render_reference(&hit).ok()?;
            characters += reference.chars().count();
            hits.push(ExplainedMemoryHit {
                hit,
                explanation: cached.explanation.clone(),
                reference,
            });
        }
        if characters > settings.max_characters || characters != self.diagnostics.used_characters {
            return None;
        }
        Some(HybridSearchResult {
            hits,
            diagnostics: self.diagnostics.clone(),
        })
    }
}
