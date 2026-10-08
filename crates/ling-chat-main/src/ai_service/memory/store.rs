//! `memory_embeddings` 表的读写：分块、向量序列化、按角色增删查。

use anyhow::{Context, Result};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};

use crate::db::entities::memory_embedding;

/// 分块字符数与重叠字符数（对齐 BaiShou 的 1024/128，这里按字符而非 token）。
const CHUNK_CHARS: usize = 1024;
const CHUNK_OVERLAP_CHARS: usize = 128;

/// 检索/去重时从库里读出的一行。
#[derive(Clone, Debug)]
pub struct MemoryRow {
    pub id: i32,
    pub memory_id: String,
    pub chunk_index: i32,
    pub content: String,
    pub tags: Option<String>,
    /// L2 归一化后的向量。
    pub embedding: Vec<f32>,
    /// 产生该向量的嵌入模型名。
    pub model: String,
    pub created_at: i64,
}

/// 把文本切成 ~1024 字符、相邻块重叠 128 字符的分块；空文本返回单块。
pub fn chunk_text(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= CHUNK_CHARS {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < chars.len() {
        let end = (start + CHUNK_CHARS).min(chars.len());
        chunks.push(chars[start..end].iter().collect());
        if end == chars.len() {
            break;
        }
        let step = (CHUNK_CHARS - CHUNK_OVERLAP_CHARS).max(1);
        start += step;
    }
    chunks
}

/// 一次写入一张记忆的全部分块（同一 `memory_id`）。
pub async fn insert_memory(
    db: &DatabaseConnection,
    role_id: i32,
    memory_id: &str,
    chunks: &[String],
    embeddings: &[Vec<f32>],
    tags: Option<String>,
    model: &str,
) -> Result<()> {
    let created_at = chrono::Utc::now().timestamp_millis();
    for (index, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
        let model_row = memory_embedding::ActiveModel {
            role_id: sea_orm::ActiveValue::Set(role_id),
            memory_id: sea_orm::ActiveValue::Set(memory_id.to_string()),
            chunk_index: sea_orm::ActiveValue::Set(index as i32),
            content: sea_orm::ActiveValue::Set(chunk.clone()),
            tags: sea_orm::ActiveValue::Set(tags.clone()),
            embedding: sea_orm::ActiveValue::Set(f32_to_bytes(embedding)),
            dim: sea_orm::ActiveValue::Set(embedding.len() as i32),
            model: sea_orm::ActiveValue::Set(model.to_string()),
            created_at: sea_orm::ActiveValue::Set(created_at),
            ..Default::default()
        };
        memory_embedding::Entity::insert(model_row)
            .exec(db)
            .await
            .context("写入记忆向量失败")?;
    }
    Ok(())
}

/// 删除一张记忆的全部分块，返回删除行数。
pub async fn delete_memory(db: &DatabaseConnection, memory_id: &str) -> Result<usize> {
    let deleted = memory_embedding::Entity::delete_many()
        .filter(memory_embedding::Column::MemoryId.eq(memory_id.to_string()))
        .exec(db)
        .await?;
    Ok(deleted.rows_affected as usize)
}

/// 同上，但只允许删指定角色名下的记忆（越权保护）。
pub async fn delete_memory_scoped(
    db: &DatabaseConnection,
    role_id: i32,
    memory_id: &str,
) -> Result<usize> {
    let deleted = memory_embedding::Entity::delete_many()
        .filter(
            memory_embedding::Column::MemoryId
                .eq(memory_id.to_string())
                .and(memory_embedding::Column::RoleId.eq(role_id)),
        )
        .exec(db)
        .await?;
    Ok(deleted.rows_affected as usize)
}

/// 某角色当前的记忆条数（按 `memory_id` 去重，分块不重复计）。
pub async fn count_role_memories(db: &DatabaseConnection, role_id: i32) -> Result<usize> {
    let ids: Vec<String> = memory_embedding::Entity::find()
        .select_only()
        .column(memory_embedding::Column::MemoryId)
        .distinct()
        .filter(memory_embedding::Column::RoleId.eq(role_id))
        .into_tuple()
        .all(db)
        .await?;
    Ok(ids.len())
}

/// 存在长期记忆的角色 ID 列表（供压缩入口遍历所有角色）。
pub async fn role_ids_with_memories(db: &DatabaseConnection) -> Result<Vec<i32>> {
    let role_ids: Vec<i32> = memory_embedding::Entity::find()
        .select_only()
        .column(memory_embedding::Column::RoleId)
        .distinct()
        .into_tuple()
        .all(db)
        .await?;
    Ok(role_ids)
}

/// 读取某角色的全部记忆分块（按记忆、分块序排好）。
pub async fn load_role_rows(db: &DatabaseConnection, role_id: i32) -> Result<Vec<MemoryRow>> {
    let rows = memory_embedding::Entity::find()
        .filter(memory_embedding::Column::RoleId.eq(role_id))
        .all(db)
        .await?;
    let mut out: Vec<MemoryRow> = rows
        .into_iter()
        .map(|row| MemoryRow {
            id: row.id,
            memory_id: row.memory_id,
            chunk_index: row.chunk_index,
            content: row.content,
            tags: row.tags,
            embedding: bytes_to_f32(&row.embedding),
            model: row.model,
            created_at: row.created_at,
        })
        .collect();
    out.sort_by(|a, b| {
        a.memory_id
            .cmp(&b.memory_id)
            .then(a.chunk_index.cmp(&b.chunk_index))
    });
    Ok(out)
}

fn f32_to_bytes(values: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * 4);
    for value in values {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn bytes_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
