//! 记忆索引的持久化入口；读取时过滤失效来源，提交时防止旧任务覆盖新索引。

use std::collections::{HashMap, HashSet};

use anyhow::{Result, ensure};
use ling_chat_notes::memory::{
    EmbeddingVersion, IndexedMemoryChunk, IndexedMemoryDocument, MemoryChunk, MemoryDocument,
    MemoryEmbeddingRecord, MemoryScope, MemoryScopeKey, memory_version,
};
use sea_orm::sea_query::OnConflict;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};

use super::save_repo::SaveRepo;
use crate::ai_service::game_system::memory::scope::MemoryScopeSnapshot;
use crate::db::entities::{memory_chunk, memory_document, memory_embedding, memory_index_state};

#[derive(Debug)]
pub struct MemoryIndexSnapshot {
    pub generation: i64,
    pub scope_key: Option<MemoryScopeKey>,
    pub index_version: Option<String>,
    pub documents: Vec<IndexedMemoryDocument>,
}

pub struct MemoryIndexRepo;

impl MemoryIndexRepo {
    pub async fn load(
        db: &DatabaseConnection,
        scope: &MemoryScopeSnapshot,
        model: &EmbeddingVersion,
    ) -> Result<MemoryIndexSnapshot> {
        let transaction = db.begin().await?;
        let key = scope.key();
        let state = memory_index_state::Entity::find_by_id((key.save_id, key.role_id))
            .one(&transaction)
            .await?;
        let generation = state.as_ref().map_or(0, |state| state.generation);
        let (scope_key, index_version) = if let Some(state) = state {
            (
                serde_json::from_str(&state.scope_key).ok(),
                Some(state.index_version),
            )
        } else {
            (None, None)
        };

        let rows = memory_document::Entity::find()
            .filter(memory_document::Column::RoleId.eq(key.role_id))
            .filter(
                memory_document::Column::SaveId
                    .eq(key.save_id)
                    .or(memory_document::Column::SaveId.is_null()),
            )
            .all(&transaction)
            .await?;
        let mut documents = HashMap::new();
        for row in rows {
            if let Ok(document) = serde_json::from_str::<MemoryDocument>(&row.payload) {
                let save_id = match document.scope {
                    MemoryScope::Save { save_id } => Some(save_id),
                    MemoryScope::Role => None,
                };
                if document.id == row.id
                    && document.version == row.version
                    && document.role_id == row.role_id
                    && save_id == row.save_id
                    && memory_kind_name(document.kind) == row.kind
                    && scope.allows_document(&document)
                {
                    documents.insert(
                        document.id.clone(),
                        IndexedMemoryDocument {
                            document,
                            chunking_version: row.chunking_version,
                            chunks: vec![],
                        },
                    );
                }
            }
        }

        let document_ids: Vec<String> = documents.keys().cloned().collect();
        let chunks = if document_ids.is_empty() {
            Vec::new()
        } else {
            memory_chunk::Entity::find()
                .filter(memory_chunk::Column::DocumentId.is_in(document_ids))
                .order_by_asc(memory_chunk::Column::Ordinal)
                .all(&transaction)
                .await?
        };
        let chunk_ids: Vec<String> = chunks.iter().map(|chunk| chunk.id.clone()).collect();
        let embedding_rows = if chunk_ids.is_empty() {
            Vec::new()
        } else {
            memory_embedding::Entity::find()
                .filter(memory_embedding::Column::ChunkId.is_in(chunk_ids))
                .all(&transaction)
                .await?
        };
        let embeddings: HashMap<_, _> = embedding_rows
            .into_iter()
            .map(|embedding| (embedding.chunk_id.clone(), embedding))
            .collect();

        let expected_model_version = memory_version(model)?;
        for row in chunks {
            let Ok(chunk) = serde_json::from_str::<MemoryChunk>(&row.payload) else {
                continue;
            };
            let Some(document) = documents.get_mut(&chunk.document_id) else {
                continue;
            };
            if chunk.id != row.id
                || chunk.version != row.version
                || chunk.document_id != row.document_id
                || i64::from(chunk.ordinal) != row.ordinal
                || !scope.allows_chunk(&document.document, &chunk)
            {
                continue;
            }
            let embedding = embeddings
                .get(&chunk.id)
                .and_then(|embedding| {
                    serde_json::from_str::<MemoryEmbeddingRecord>(&embedding.payload).ok()
                })
                .filter(|embedding| {
                    embedding.model == *model
                        && embedding.validate_for(&chunk).is_ok()
                        && embeddings
                            .get(&chunk.id)
                            .is_some_and(|stored| stored.chunk_version == chunk.version)
                        && embeddings
                            .get(&chunk.id)
                            .is_some_and(|stored| stored.model_version == expected_model_version)
                });
            document
                .chunks
                .push(IndexedMemoryChunk { chunk, embedding });
        }
        transaction.commit().await?;
        let mut documents: Vec<_> = documents.into_values().collect();
        documents.sort_by(|left, right| left.document.id.cmp(&right.document.id));
        Ok(MemoryIndexSnapshot {
            generation,
            scope_key,
            index_version,
            documents,
        })
    }

    pub(crate) async fn commit(
        db: &DatabaseConnection,
        scope: &MemoryScopeSnapshot,
        expected_generation: i64,
        index_version: &str,
        desired: &[IndexedMemoryDocument],
        changed: &HashSet<String>,
    ) -> Result<()> {
        let key = scope.key();
        ensure!(
            desired
                .iter()
                .map(|item| &item.document.id)
                .collect::<HashSet<_>>()
                .len()
                == desired.len(),
            "文档身份重复"
        );
        for item in desired {
            ensure!(
                scope.allows_document(&item.document),
                "索引文档来源已经失效"
            );
            let mut ids = HashSet::new();
            let mut ordinals = HashSet::new();
            for item_chunk in &item.chunks {
                ensure!(
                    scope.allows_chunk(&item.document, &item_chunk.chunk),
                    "索引分块来源已经失效"
                );
                ensure!(
                    ids.insert(&item_chunk.chunk.id) && ordinals.insert(item_chunk.chunk.ordinal),
                    "分块身份重复"
                );
                if let Some(embedding) = &item_chunk.embedding {
                    embedding.validate_for(&item_chunk.chunk)?;
                }
            }
        }
        let transaction = db.begin().await?;
        ensure!(
            generation(&transaction, key).await? == expected_generation,
            "索引已被其他任务更新，请重试"
        );
        let mut history = SaveRepo::get_memory_gameline_list(&transaction, key.save_id).await?;
        ensure!(
            MemoryScopeSnapshot::history_version_for(key.save_id, key.role_id, &mut history)?
                == key.history_version,
            "历史已变化，丢弃旧索引任务"
        );
        let rows = memory_document::Entity::find()
            .filter(memory_document::Column::RoleId.eq(key.role_id))
            .filter(
                memory_document::Column::SaveId
                    .eq(key.save_id)
                    .or(memory_document::Column::SaveId.is_null()),
            )
            .all(&transaction)
            .await?;
        let desired_ids: HashSet<_> = desired
            .iter()
            .map(|item| item.document.id.as_str())
            .collect();
        for row in rows {
            if !desired_ids.contains(row.id.as_str()) {
                delete_document(&transaction, &row.id).await?;
            }
        }
        for item in desired
            .iter()
            .filter(|item| changed.contains(&item.document.id))
        {
            delete_document(&transaction, &item.document.id).await?;
            let save_id = match item.document.scope {
                MemoryScope::Save { save_id } => Some(save_id),
                MemoryScope::Role => None,
            };
            memory_document::Entity::insert(memory_document::ActiveModel {
                id: Set(item.document.id.clone()),
                role_id: Set(item.document.role_id),
                save_id: Set(save_id),
                kind: Set(memory_kind_name(item.document.kind).to_owned()),
                version: Set(item.document.version.clone()),
                payload: Set(serde_json::to_string(&item.document)?),
                chunking_version: Set(item.chunking_version.clone()),
            })
            .exec(&transaction)
            .await?;
            for item_chunk in &item.chunks {
                let chunk = &item_chunk.chunk;
                memory_chunk::Entity::insert(memory_chunk::ActiveModel {
                    id: Set(chunk.id.clone()),
                    document_id: Set(chunk.document_id.clone()),
                    ordinal: Set(i64::from(chunk.ordinal)),
                    version: Set(chunk.version.clone()),
                    payload: Set(serde_json::to_string(chunk)?),
                })
                .exec(&transaction)
                .await?;
                if let Some(embedding) = &item_chunk.embedding {
                    memory_embedding::Entity::insert(memory_embedding::ActiveModel {
                        chunk_id: Set(chunk.id.clone()),
                        chunk_version: Set(chunk.version.clone()),
                        model_version: Set(memory_version(&embedding.model)?),
                        payload: Set(serde_json::to_string(embedding)?),
                    })
                    .exec(&transaction)
                    .await?;
                }
            }
        }
        memory_index_state::Entity::insert(memory_index_state::ActiveModel {
            save_id: Set(key.save_id),
            role_id: Set(key.role_id),
            generation: Set(expected_generation + 1),
            scope_key: Set(serde_json::to_string(key)?),
            index_version: Set(index_version.to_owned()),
        })
        .on_conflict(
            OnConflict::columns([
                memory_index_state::Column::SaveId,
                memory_index_state::Column::RoleId,
            ])
            .update_columns([
                memory_index_state::Column::Generation,
                memory_index_state::Column::ScopeKey,
                memory_index_state::Column::IndexVersion,
            ])
            .to_owned(),
        )
        .exec(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(crate) async fn delete_for_role(db: &DatabaseConnection, role_id: i32) -> Result<()> {
        let transaction = db.begin().await?;
        let documents = memory_document::Entity::find()
            .filter(memory_document::Column::RoleId.eq(role_id))
            .all(&transaction)
            .await?;
        for document in documents {
            delete_document(&transaction, &document.id).await?;
        }
        memory_index_state::Entity::delete_many()
            .filter(memory_index_state::Column::RoleId.eq(role_id))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(crate) async fn delete_for_save(db: &DatabaseConnection, save_id: i32) -> Result<()> {
        let transaction = db.begin().await?;
        let documents = memory_document::Entity::find()
            .filter(memory_document::Column::SaveId.eq(save_id))
            .all(&transaction)
            .await?;
        for document in documents {
            delete_document(&transaction, &document.id).await?;
        }
        memory_index_state::Entity::delete_many()
            .filter(memory_index_state::Column::SaveId.eq(save_id))
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

fn memory_kind_name(kind: ling_chat_notes::memory::MemoryKind) -> &'static str {
    match kind {
        ling_chat_notes::memory::MemoryKind::Raw => "raw",
        ling_chat_notes::memory::MemoryKind::Diary => "diary",
        ling_chat_notes::memory::MemoryKind::Note => "note",
    }
}

async fn generation(db: &impl sea_orm::ConnectionTrait, key: &MemoryScopeKey) -> Result<i64> {
    Ok(
        memory_index_state::Entity::find_by_id((key.save_id, key.role_id))
            .one(db)
            .await?
            .map(|state| state.generation)
            .unwrap_or(0),
    )
}

// 按依赖顺序显式删除，兼容未启用 SQLite 外键级联的已有连接。
async fn delete_document(db: &impl sea_orm::ConnectionTrait, id: &str) -> Result<()> {
    let chunks = memory_chunk::Entity::find()
        .filter(memory_chunk::Column::DocumentId.eq(id.to_owned()))
        .all(db)
        .await?;
    let chunk_ids: Vec<String> = chunks.into_iter().map(|chunk| chunk.id).collect();
    if !chunk_ids.is_empty() {
        memory_embedding::Entity::delete_many()
            .filter(memory_embedding::Column::ChunkId.is_in(chunk_ids))
            .exec(db)
            .await?;
    }
    memory_chunk::Entity::delete_many()
        .filter(memory_chunk::Column::DocumentId.eq(id.to_owned()))
        .exec(db)
        .await?;
    memory_document::Entity::delete_by_id(id.to_owned())
        .exec(db)
        .await?;
    Ok(())
}
