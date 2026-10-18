use std::collections::{HashMap, HashSet};

use anyhow::{Result, ensure};
use ling_chat_notes::memory::{
    EmbeddingVersion, IndexedMemoryChunk, IndexedMemoryDocument, MemoryChunk, MemoryDocument,
    MemoryEmbeddingRecord, MemoryScope, MemoryScopeKey, memory_version,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value};

use super::save_repo::SaveRepo;
use crate::ai_service::game_system::memory_scope::MemoryScopeSnapshot;

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
        let generation = generation(&transaction, key).await?;
        let state = transaction.query_one(statement(
            "SELECT scope_key,index_version FROM memory_index_state WHERE save_id=? AND role_id=?",
            vec![key.save_id.into(), key.role_id.into()],
        )).await?;
        let (scope_key, index_version) = if let Some(row) = state {
            (
                serde_json::from_str(&row.try_get_by_index::<String>(0)?).ok(),
                Some(row.try_get_by_index(1)?),
            )
        } else {
            (None, None)
        };
        let rows = transaction.query_all(statement(
            "SELECT id,version,payload,role_id,save_id,kind,chunking_version FROM memory_document WHERE role_id=? AND (save_id=? OR save_id IS NULL)",
            vec![key.role_id.into(), key.save_id.into()],
        )).await?;
        let mut documents = HashMap::new();
        for row in rows {
            let payload: String = row.try_get_by_index(2)?;
            if let Ok(document) = serde_json::from_str::<MemoryDocument>(&payload) {
                let save_id = match document.scope {
                    MemoryScope::Save { save_id } => Some(save_id),
                    MemoryScope::Role => None,
                };
                if document.id == row.try_get_by_index::<String>(0)?
                    && document.version == row.try_get_by_index::<String>(1)?
                    && document.role_id == row.try_get_by_index::<i32>(3)?
                    && save_id == row.try_get_by_index::<Option<i32>>(4)?
                    && serde_json::to_value(document.kind)?.as_str()
                        == Some(row.try_get_by_index::<String>(5)?.as_str())
                    && scope.allows_document(&document)
                {
                    documents.insert(
                        document.id.clone(),
                        IndexedMemoryDocument {
                            document,
                            chunking_version: row.try_get_by_index(6)?,
                            chunks: vec![],
                        },
                    );
                }
            }
        }
        let rows = transaction.query_all(statement(
            "SELECT c.id,c.version,c.payload,e.payload,c.document_id,e.chunk_version,e.model_version,c.ordinal FROM memory_chunk c
             JOIN memory_document d ON d.id=c.document_id
             LEFT JOIN memory_embedding e ON e.chunk_id=c.id
             WHERE d.role_id=? AND (d.save_id=? OR d.save_id IS NULL) ORDER BY c.ordinal",
            vec![key.role_id.into(), key.save_id.into()],
        )).await?;
        let expected_model_version = memory_version(model)?;
        for row in rows {
            let payload: String = row.try_get_by_index(2)?;
            let Ok(chunk) = serde_json::from_str::<MemoryChunk>(&payload) else {
                continue;
            };
            let Some(document) = documents.get_mut(&chunk.document_id) else {
                continue;
            };
            if chunk.id != row.try_get_by_index::<String>(0)?
                || chunk.version != row.try_get_by_index::<String>(1)?
                || chunk.document_id != row.try_get_by_index::<String>(4)?
                || i64::from(chunk.ordinal) != row.try_get_by_index::<i64>(7)?
                || !scope.allows_chunk(&document.document, &chunk)
            {
                continue;
            }
            let stored_chunk_version: Option<String> = row.try_get_by_index(5)?;
            let stored_model_version: Option<String> = row.try_get_by_index(6)?;
            let embedding = row
                .try_get_by_index::<Option<String>>(3)?
                .and_then(|payload| serde_json::from_str::<MemoryEmbeddingRecord>(&payload).ok())
                .filter(|embedding| {
                    embedding.model == *model
                        && embedding.validate_for(&chunk).is_ok()
                        && stored_chunk_version.as_ref() == Some(&chunk.version)
                        && stored_model_version.as_ref() == Some(&expected_model_version)
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
        let rows = transaction
            .query_all(statement(
                "SELECT id FROM memory_document WHERE role_id=? AND (save_id=? OR save_id IS NULL)",
                vec![key.role_id.into(), key.save_id.into()],
            ))
            .await?;
        let desired_ids: HashSet<_> = desired
            .iter()
            .map(|item| item.document.id.as_str())
            .collect();
        for row in rows {
            let id: String = row.try_get_by_index(0)?;
            if !desired_ids.contains(id.as_str()) {
                delete_document(&transaction, &id).await?;
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
            transaction.execute(statement(
                "INSERT INTO memory_document(id,role_id,save_id,kind,version,payload,chunking_version) VALUES (?,?,?,?,?,?,?)",
                vec![item.document.id.clone().into(), item.document.role_id.into(), save_id.into(),
                    serde_json::to_value(item.document.kind)?.as_str().unwrap().to_owned().into(),
                    item.document.version.clone().into(), serde_json::to_string(&item.document)?.into(),
                    item.chunking_version.clone().into()],
            )).await?;
            for item_chunk in &item.chunks {
                let chunk = &item_chunk.chunk;
                transaction.execute(statement(
                    "INSERT INTO memory_chunk(id,document_id,ordinal,version,payload) VALUES (?,?,?,?,?)",
                    vec![chunk.id.clone().into(), chunk.document_id.clone().into(), i64::from(chunk.ordinal).into(),
                        chunk.version.clone().into(), serde_json::to_string(chunk)?.into()],
                )).await?;
                if let Some(embedding) = &item_chunk.embedding {
                    transaction.execute(statement(
                        "INSERT INTO memory_embedding(chunk_id,chunk_version,model_version,payload) VALUES (?,?,?,?)",
                        vec![chunk.id.clone().into(), chunk.version.clone().into(), memory_version(&embedding.model)?.into(),
                            serde_json::to_string(embedding)?.into()],
                    )).await?;
                }
            }
        }
        transaction.execute(statement(
            "INSERT INTO memory_index_state(save_id,role_id,generation,scope_key,index_version) VALUES (?,?,?,?,?)
             ON CONFLICT(save_id,role_id) DO UPDATE SET generation=excluded.generation,scope_key=excluded.scope_key,index_version=excluded.index_version",
            vec![key.save_id.into(), key.role_id.into(), (expected_generation + 1).into(),
                serde_json::to_string(key)?.into(), index_version.to_owned().into()],
        )).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(crate) async fn delete_for_role(db: &DatabaseConnection, role_id: i32) -> Result<()> {
        Self::delete_scope(db, "role_id", role_id).await
    }

    pub(crate) async fn delete_for_save(db: &DatabaseConnection, save_id: i32) -> Result<()> {
        Self::delete_scope(db, "save_id", save_id).await
    }

    async fn delete_scope(db: &DatabaseConnection, column: &str, identity: i32) -> Result<()> {
        let transaction = db.begin().await?;
        let rows = transaction
            .query_all(statement(
                &format!("SELECT id FROM memory_document WHERE {column}=?"),
                vec![identity.into()],
            ))
            .await?;
        for row in rows {
            delete_document(&transaction, &row.try_get_by_index::<String>(0)?).await?;
        }
        transaction
            .execute(statement(
                &format!("DELETE FROM memory_index_state WHERE {column}=?"),
                vec![identity.into()],
            ))
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

fn statement(sql: &str, values: Vec<Value>) -> Statement {
    Statement::from_sql_and_values(DbBackend::Sqlite, sql, values)
}

async fn generation(db: &impl ConnectionTrait, key: &MemoryScopeKey) -> Result<i64> {
    Ok(db
        .query_one(statement(
            "SELECT generation FROM memory_index_state WHERE save_id=? AND role_id=?",
            vec![key.save_id.into(), key.role_id.into()],
        ))
        .await?
        .map(|row| row.try_get_by_index(0))
        .transpose()?
        .unwrap_or(0))
}

async fn delete_document(db: &impl ConnectionTrait, id: &str) -> Result<()> {
    db.execute(statement("DELETE FROM memory_embedding WHERE chunk_id IN (SELECT id FROM memory_chunk WHERE document_id=?)", vec![id.to_owned().into()])).await?;
    db.execute(statement(
        "DELETE FROM memory_chunk WHERE document_id=?",
        vec![id.to_owned().into()],
    ))
    .await?;
    db.execute(statement(
        "DELETE FROM memory_document WHERE id=?",
        vec![id.to_owned().into()],
    ))
    .await?;
    Ok(())
}
