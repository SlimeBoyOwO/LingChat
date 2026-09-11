//! 语义记忆的持久化向量库（SQLite）。
//!
//! 独立于普通记忆库：向量、文本、标签按 `role_id` 分库存在
//! `data/game_data/semantic_memory.db`。查询时读出该角色的全部向量，
//! 在 Rust 侧做余弦相似度（当前规模为数千片段，暴力检索足够；向量在写入
//! 前已被服务层归一化，因此余弦 = 点积）。

use std::path::Path;

use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

/// 一条持久化的语义记忆记录。
#[derive(Debug, Clone)]
pub struct StoredMemory {
    pub id: String,
    pub text: String,
    pub vector: Vec<f32>,
    pub tags: String,
    pub created_at: String,
}

/// SQLite 向量库句柄。连接池只保留 1 个连接，避免并发写库争用。
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    /// 打开（必要时创建）向量库并建立表结构。失败时返回可读错误。
    pub async fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("创建语义记忆数据目录失败: {e}"))?;
        }
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| format!("打开语义记忆向量库失败: {e}"))?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS semantic_memory (
                id         TEXT PRIMARY KEY,
                role_id    INTEGER NOT NULL,
                text       TEXT NOT NULL,
                dim        INTEGER NOT NULL,
                vector     BLOB NOT NULL,
                tags       TEXT NOT NULL DEFAULT '[]',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("初始化语义记忆表失败: {e}"))?;
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_semantic_memory_role
             ON semantic_memory (role_id)",
        )
        .execute(&pool)
        .await
        .map_err(|e| format!("初始化语义记忆索引失败: {e}"))?;

        Ok(Self { pool })
    }

    /// 写入一条记忆。
    pub async fn insert(
        &self,
        id: &str,
        role_id: i32,
        text: &str,
        dim: usize,
        vector: &[f32],
        tags: &str,
        ts: &str,
    ) -> Result<(), String> {
        sqlx::query(
            "INSERT INTO semantic_memory (id, role_id, text, dim, vector, tags, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(role_id)
        .bind(text)
        .bind(dim as i64)
        .bind(encode_vector(vector))
        .bind(tags)
        .bind(ts)
        .bind(ts)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("写入语义记忆失败: {e}"))?;
        Ok(())
    }

    /// 取出某个角色的全部记忆（含向量，供去重/检索）。
    pub async fn fetch_role(&self, role_id: i32) -> Result<Vec<StoredMemory>, String> {
        let rows = sqlx::query(
            "SELECT id, text, vector, tags, created_at FROM semantic_memory WHERE role_id = ?",
        )
        .bind(role_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| format!("读取语义记忆失败: {e}"))?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(StoredMemory {
                id: row.get(0),
                text: row.get(1),
                vector: decode_vector(row.get::<Vec<u8>, _>(2)),
                tags: row.get(3),
                created_at: row.get(4),
            });
        }
        Ok(out)
    }

    /// 按 id 删除某个角色的记忆。返回是否实际删除。
    pub async fn delete(&self, role_id: i32, id: &str) -> Result<bool, String> {
        let result = sqlx::query("DELETE FROM semantic_memory WHERE role_id = ? AND id = ?")
            .bind(role_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("删除语义记忆失败: {e}"))?;
        Ok(result.rows_affected() > 0)
    }

    /// 更新一条记忆的文本与向量（重新编码后调用）。返回是否实际更新。
    pub async fn update(
        &self,
        role_id: i32,
        id: &str,
        text: &str,
        dim: usize,
        vector: &[f32],
        ts: &str,
    ) -> Result<bool, String> {
        let result = sqlx::query(
            "UPDATE semantic_memory SET text = ?, dim = ?, vector = ?, updated_at = ?
             WHERE role_id = ? AND id = ?",
        )
        .bind(text)
        .bind(dim as i64)
        .bind(encode_vector(vector))
        .bind(ts)
        .bind(role_id)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| format!("更新语义记忆失败: {e}"))?;
        Ok(result.rows_affected() > 0)
    }

    /// 按文本删除某个角色的记忆（AI 未持有 id 时的兜底）。返回是否实际删除。
    pub async fn delete_by_text(&self, role_id: i32, text: &str) -> Result<bool, String> {
        let result = sqlx::query("DELETE FROM semantic_memory WHERE role_id = ? AND text = ?")
            .bind(role_id)
            .bind(text)
            .execute(&self.pool)
            .await
            .map_err(|e| format!("删除语义记忆失败: {e}"))?;
        Ok(result.rows_affected() > 0)
    }

    /// 全局记忆总数（供前端状态面板展示）。
    pub async fn count_all(&self) -> usize {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM semantic_memory")
            .fetch_one(&self.pool)
            .await
            .unwrap_or(0);
        count.max(0) as usize
    }
}

fn encode_vector(vector: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn decode_vector(bytes: Vec<u8>) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}
