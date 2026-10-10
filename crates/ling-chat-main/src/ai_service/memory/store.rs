//! 长期记忆存储：写入每「剧本 × 角色」一张的共用归档表 `memory_archive_{role_id}`，
//! 目录信息在 `memory_archive_profiles`。
//!
//! 共用的归档表同时容纳两类条目：
//! - `kind = 'legacy'`：旧版存档记忆整合导出的四段式条目（无向量、无 `memory_id`，
//!   只被「旧版记忆整合」面板访问）；
//! - `kind = 'memory'`：长期记忆（带向量，按 `memory_id` 分块，供 RAG 检索 / 浏览 /
//!   删除 / 摘要使用）。
//! 带向量的行走「向量 + 关键词」混合检索；纯文本行在混合模式下只会被关键词路命中。

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection,
    EntityTrait, FromQueryResult, QueryFilter, Set, Statement,
};

use crate::db::entities::{memory_archive_profile, role};

/// 分块字符数与重叠字符数（对齐 BaiShou 的 1024/128，这里按字符而非 token）。
const CHUNK_CHARS: usize = 1024;
const CHUNK_OVERLAP_CHARS: usize = 128;

/// 共用归档数据表 `memory_archive_{role_id}` 的建表语句（动态表名）。
///
/// `memory` 行有向量（`embedding` / `dim` / `model` 非空、`memory_id` 非空）；
/// `legacy` 行是纯文本（向量三列与 `memory_id` 为 NULL，`chunk_index` 为 0）。
pub const MEMORY_ARCHIVE_DDL: &str = r#"CREATE TABLE IF NOT EXISTS "{table}" (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL DEFAULT 'legacy',
  memory_id TEXT,
  chunk_index INTEGER NOT NULL DEFAULT 0,
  section TEXT NOT NULL,
  content TEXT NOT NULL,
  embedding BLOB,
  dim INTEGER,
  model TEXT,
  created_at INTEGER NOT NULL
)"#;

fn table_name(role_id: i32) -> String {
    format!("memory_archive_{role_id}")
}

/// 确保某角色的归档数据表存在且结构最新：
/// 1. `CREATE TABLE IF NOT EXISTS`（新结构，含向量列）；
/// 2. 对历史版本建的旧结构表（只有 id/section/content/created_at）用
///    `ALTER TABLE ADD COLUMN` 逐列补齐（幂等，无 `IF NOT EXISTS` 时先查表结构）。
pub async fn ensure_archive_table_schema(
    db: &DatabaseConnection,
    table: &str,
) -> Result<(), String> {
    #[derive(Debug, FromQueryResult)]
    struct ColName {
        #[sea_orm(column_name = "name")]
        name: String,
    }

    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        MEMORY_ARCHIVE_DDL.replace("{table}", table),
    ))
    .await
    .map_err(|e| format!("确保归档表 {table} 存在失败: {e}"))?;

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!("SELECT name FROM pragma_table_info('{table}')"),
        ))
        .await
        .map_err(|e| format!("读取归档表结构失败: {e}"))?;
    let mut cols: HashSet<String> = rows
        .iter()
        .filter_map(|row| ColName::from_query_result(row, "").ok().map(|c| c.name))
        .collect();

    const MISSING: [(&str, &str); 6] = [
        ("kind", "TEXT NOT NULL DEFAULT 'legacy'"),
        ("memory_id", "TEXT"),
        ("chunk_index", "INTEGER NOT NULL DEFAULT 0"),
        ("embedding", "BLOB"),
        ("dim", "INTEGER"),
        ("model", "TEXT"),
    ];
    for (name, ddl) in MISSING {
        if !cols.contains(name) {
            db.execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!("ALTER TABLE \"{table}\" ADD COLUMN {name} {ddl}"),
            ))
            .await
            .map_err(|e| format!("归档表 {table} 补列 {name} 失败: {e}"))?;
            cols.insert(name.to_string());
        }
    }
    Ok(())
}

/// 确保某角色已有归档档案：目录（`memory_archive_profiles`）里没有则新建档案
/// （数据表 + 目录行），返回该角色的归档表名。
///
/// 与 `api::memory_archive::memory_import_legacy` 共用——长期记忆与旧版整合
/// 都落到同一张「剧本 × 角色」表。
pub async fn ensure_role_archive(db: &DatabaseConnection, role_id: i32) -> Result<String, String> {
    let exist = memory_archive_profile::Entity::find()
        .filter(memory_archive_profile::Column::RoleId.eq(role_id))
        .one(db)
        .await
        .map_err(|e| format!("查询档案目录失败: {e}"))?;
    if let Some(profile) = exist {
        ensure_archive_table_schema(db, &profile.table_name).await?;
        return Ok(profile.table_name);
    }

    let (name, script_key, script_role_key) = role::Entity::find_by_id(role_id)
        .one(db)
        .await
        .map_err(|e| format!("读取角色失败: {e}"))?
        .map(|r| (r.name, r.script_key, r.script_role_key))
        .unwrap_or_default();

    let table = table_name(role_id);
    ensure_archive_table_schema(db, &table).await?;

    let now = chrono::Utc::now().timestamp_millis();
    memory_archive_profile::ActiveModel {
        script_key: Set(script_key),
        script_role_key: Set(script_role_key),
        role_id: Set(role_id),
        role_name: Set(if name.is_empty() {
            format!("角色 {role_id}")
        } else {
            name
        }),
        table_name: Set(table.clone()),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(|e| format!("写入档案目录失败: {e}"))?;

    Ok(table)
}

/// 某角色归档表里 `kind='memory'` 的记忆条数（按 `memory_id` 去重，分块不重复计）。
async fn count_role_memory_rows(db: &DatabaseConnection, table: &str) -> Result<usize> {
    #[derive(Debug, FromQueryResult)]
    struct CountRow {
        #[sea_orm(column_name = "c")]
        c: i64,
    }
    let count = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT COUNT(DISTINCT memory_id) AS c FROM \"{table}\" \
                 WHERE kind='memory' AND memory_id IS NOT NULL"
            ),
        ))
        .await?
        .and_then(|row| CountRow::from_query_result(&row, "").ok())
        .map(|c| c.c)
        .unwrap_or(0);
    Ok(count as usize)
}

/// 一次写入一张长期记忆的全部分块（同一 `memory_id`，`kind='memory'`）。
/// `tags` 由内容尾部 `\n[标签: …]` 行承载，归档表不再单独存标签列。
pub async fn insert_memory(
    db: &DatabaseConnection,
    role_id: i32,
    memory_id: &str,
    chunks: &[String],
    embeddings: &[Vec<f32>],
    _tags: Option<String>,
    model: &str,
) -> Result<()> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let created_at = chrono::Utc::now().timestamp_millis();
    for (index, (chunk, embedding)) in chunks.iter().zip(embeddings.iter()).enumerate() {
        db.execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!(
                "INSERT INTO \"{table}\" \
                 (kind, memory_id, chunk_index, section, content, embedding, dim, model, created_at) \
                 VALUES ('memory', ?, ?, 'long_term', ?, ?, ?, ?, ?)"
            ),
            vec![
                sea_orm::Value::String(Some(Box::new(memory_id.to_string()))),
                sea_orm::Value::Int(Some(index as i32)),
                sea_orm::Value::String(Some(Box::new(chunk.clone()))),
                sea_orm::Value::Bytes(Some(Box::new(f32_to_bytes(embedding)))),
                sea_orm::Value::Int(Some(embedding.len() as i32)),
                sea_orm::Value::String(Some(Box::new(model.to_string()))),
                sea_orm::Value::BigInt(Some(created_at)),
            ],
        ))
        .await
        .context("写入记忆向量失败")?;
    }
    Ok(())
}

/// 删除某角色名下的一张长期记忆的全部分块（越权保护），返回删除行数。
pub async fn delete_memory_scoped(
    db: &DatabaseConnection,
    role_id: i32,
    memory_id: &str,
) -> Result<usize> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let result = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!("DELETE FROM \"{table}\" WHERE kind='memory' AND memory_id = ?"),
            vec![sea_orm::Value::String(Some(Box::new(
                memory_id.to_string(),
            )))],
        ))
        .await
        .context("删除记忆向量失败")?;
    Ok(result.rows_affected() as usize)
}

/// 某角色当前的长期记忆条数（按 `memory_id` 去重，分块不重复计）。
pub async fn count_role_memories(db: &DatabaseConnection, role_id: i32) -> Result<usize> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    count_role_memory_rows(db, &table).await
}

/// 存在长期记忆角色的 ID 列表（供压缩入口遍历所有角色）。
pub async fn role_ids_with_memories(db: &DatabaseConnection) -> Result<Vec<i32>> {
    let profiles = memory_archive_profile::Entity::find().all(db).await?;
    let mut out: Vec<i32> = Vec::new();
    for profile in profiles {
        if ensure_archive_table_schema(db, &profile.table_name)
            .await
            .is_err()
        {
            continue;
        }
        // 共用表里任一内容（长期记忆或旧版整合条目）都算「有可浏览记忆」。
        if count_table_archive_entries(db, &profile.table_name).await? > 0 {
            out.push(profile.role_id);
        }
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

/// 某角色「可浏览条目数」：长期记忆条数 + 旧版整合条目数（浏览面板下拉计数用）。
pub async fn count_role_entries(db: &DatabaseConnection, role_id: i32) -> Result<usize> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    count_table_archive_entries(db, &table).await
}

/// 共用归档表里的可浏览条目数（长期记忆按 `memory_id` 去重 + 旧版整合逐行计）。
async fn count_table_archive_entries(db: &DatabaseConnection, table: &str) -> Result<usize> {
    #[derive(Debug, FromQueryResult)]
    struct CountRow {
        #[sea_orm(column_name = "c")]
        c: i64,
    }
    let count = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT (SELECT COUNT(DISTINCT memory_id) FROM \"{table}\" \
                   WHERE kind='memory' AND memory_id IS NOT NULL) \
                 + (SELECT COUNT(*) FROM \"{table}\" WHERE kind='legacy') AS c"
            ),
        ))
        .await?
        .and_then(|row| CountRow::from_query_result(&row, "").ok())
        .map(|c| c.c)
        .unwrap_or(0);
    Ok(count as usize)
}

/// 读取某角色的旧版整合条目（`kind='legacy'`，逐行一条，按创建时间倒序）。
pub async fn load_role_legacy_rows(
    db: &DatabaseConnection,
    role_id: i32,
) -> Result<Vec<LegacyRow>> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    #[derive(Debug, FromQueryResult)]
    struct LegacyRowRaw {
        id: i32,
        content: String,
        created_at: i64,
    }

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT id, content, created_at FROM \"{table}\" \
                 WHERE kind='legacy' ORDER BY created_at DESC, id DESC"
            ),
        ))
        .await
        .context("读取旧版整合条目失败")?;

    let mut out: Vec<LegacyRow> = Vec::with_capacity(rows.len());
    for row in &rows {
        if let Ok(r) = LegacyRowRaw::from_query_result(row, "") {
            out.push(LegacyRow {
                id: r.id,
                content: r.content,
                created_at: r.created_at,
            });
        }
    }
    Ok(out)
}

/// 删除某角色共用表里的一条旧版整合条目（越权保护），返回删除行数。
pub async fn delete_legacy_row(
    db: &DatabaseConnection,
    role_id: i32,
    row_id: i32,
) -> Result<usize> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let result = db
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            format!("DELETE FROM \"{table}\" WHERE id = ? AND kind='legacy'"),
            vec![sea_orm::Value::Int(Some(row_id))],
        ))
        .await
        .context("删除旧版整合条目失败")?;
    Ok(result.rows_affected() as usize)
}

/// 读取某角色的全部长期记忆分块（`kind='memory'`，按记忆、分块序排好）。
pub async fn load_role_rows(db: &DatabaseConnection, role_id: i32) -> Result<Vec<MemoryRow>> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    #[derive(Debug, FromQueryResult)]
    struct ArchiveRow {
        id: i32,
        memory_id: Option<String>,
        chunk_index: i32,
        content: String,
        #[sea_orm(column_name = "embedding")]
        embedding: Option<Vec<u8>>,
        #[sea_orm(column_name = "model")]
        model: Option<String>,
        created_at: i64,
    }

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT id, memory_id, chunk_index, content, embedding, model, created_at \
                 FROM \"{table}\" WHERE kind='memory' ORDER BY memory_id, chunk_index"
            ),
        ))
        .await
        .context("读取记忆向量失败")?;

    let mut out: Vec<MemoryRow> = Vec::with_capacity(rows.len());
    for row in &rows {
        if let Ok(r) = ArchiveRow::from_query_result(row, "") {
            if let Some(memory_id) = r.memory_id {
                out.push(MemoryRow {
                    id: r.id,
                    memory_id,
                    chunk_index: r.chunk_index,
                    content: r.content,
                    tags: None,
                    embedding: r
                        .embedding
                        .map(|bytes| bytes_to_f32(&bytes))
                        .unwrap_or_default(),
                    model: r.model.unwrap_or_default(),
                    created_at: r.created_at,
                });
            }
        }
    }
    out.sort_by(|a, b| {
        a.memory_id
            .cmp(&b.memory_id)
            .then(a.chunk_index.cmp(&b.chunk_index))
    });
    Ok(out)
}

/// 检索用：某角色归档表里的可检索行——长期记忆（`kind='memory'`）**加上**
/// 已带向量的旧版整合条目（`kind='legacy'` 且 `embedding` 非空）。
///
/// 与 [`load_role_rows`] 的区别：这里额外纳入整合条目，供 `memory_query` 工具
/// 语义检索；自动 RAG 注入仍走 [`load_role_rows`]（只召回长期记忆）。整合行没有
/// `memory_id`，这里用 `legacy_{行id}` 表示（与浏览面板、精确删除一致），
/// `chunk_index` 统一记 0。
pub async fn load_role_search_rows(
    db: &DatabaseConnection,
    role_id: i32,
) -> Result<Vec<MemoryRow>> {
    let table = ensure_role_archive(db, role_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    #[derive(Debug, FromQueryResult)]
    struct SearchRow {
        id: i32,
        kind: String,
        memory_id: Option<String>,
        chunk_index: i32,
        content: String,
        #[sea_orm(column_name = "embedding")]
        embedding: Option<Vec<u8>>,
        #[sea_orm(column_name = "model")]
        model: Option<String>,
        created_at: i64,
    }

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT id, kind, memory_id, chunk_index, content, embedding, model, created_at \
                 FROM \"{table}\" \
                 WHERE (kind='memory' AND memory_id IS NOT NULL) \
                    OR (kind='legacy' AND embedding IS NOT NULL AND length(embedding) > 0) \
                 ORDER BY memory_id, chunk_index, id"
            ),
        ))
        .await
        .context("读取检索候选失败")?;

    let mut out: Vec<MemoryRow> = Vec::with_capacity(rows.len());
    for row in &rows {
        let Ok(r) = SearchRow::from_query_result(row, "") else {
            continue;
        };
        let memory_id = match r.memory_id {
            Some(id) => id,
            None => format!("legacy_{}", r.id),
        };
        out.push(MemoryRow {
            id: r.id,
            memory_id,
            chunk_index: if r.kind == "legacy" { 0 } else { r.chunk_index },
            content: r.content,
            tags: None,
            embedding: r
                .embedding
                .map(|bytes| bytes_to_f32(&bytes))
                .unwrap_or_default(),
            model: r.model.unwrap_or_default(),
            created_at: r.created_at,
        });
    }
    out.sort_by(|a, b| {
        a.memory_id
            .cmp(&b.memory_id)
            .then(a.chunk_index.cmp(&b.chunk_index))
    });
    Ok(out)
}

/// 一条待补算向量的旧版整合条目（含所属表名，便于回填时 UPDATE）。
#[derive(Clone, Debug)]
pub struct LegacyPendingEmbedding {
    pub table: String,
    pub row_id: i32,
    pub content: String,
}

/// 扫描（指定角色或全部角色）里还没有向量的旧版整合条目，供启动/导入后回填。
pub async fn load_legacy_pending_embeddings(
    db: &DatabaseConnection,
    role_id: Option<i32>,
) -> Result<Vec<LegacyPendingEmbedding>> {
    let profiles = memory_archive_profile::Entity::find().all(db).await?;
    let mut out: Vec<LegacyPendingEmbedding> = Vec::new();

    #[derive(Debug, FromQueryResult)]
    struct PendingRow {
        id: i32,
        content: String,
    }

    for profile in profiles {
        if role_id.is_some_and(|want| want != profile.role_id) {
            continue;
        }
        let table = match ensure_role_archive(db, profile.role_id).await {
            Ok(table) => table,
            Err(e) => {
                tracing::warn!("读取角色 {} 归档表失败，跳过向量回填: {e}", profile.role_id);
                continue;
            },
        };
        let rows = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT id, content FROM \"{table}\" \
                     WHERE kind='legacy' AND (embedding IS NULL OR length(embedding) = 0) \
                     ORDER BY id"
                ),
            ))
            .await
            .context("扫描待回填整合条目失败")?;
        for row in &rows {
            if let Ok(r) = PendingRow::from_query_result(row, "") {
                out.push(LegacyPendingEmbedding {
                    table: table.clone(),
                    row_id: r.id,
                    content: r.content,
                });
            }
        }
    }
    Ok(out)
}

/// 写入某条旧版整合条目的向量（越权保护：仅 `kind='legacy'`）。
pub async fn save_legacy_embedding(
    db: &DatabaseConnection,
    table: &str,
    row_id: i32,
    embedding: &[f32],
    model: &str,
) -> Result<()> {
    db.execute(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        format!(
            "UPDATE \"{table}\" SET embedding = ?, dim = ?, model = ? \
             WHERE id = ? AND kind='legacy'"
        ),
        vec![
            sea_orm::Value::Bytes(Some(Box::new(f32_to_bytes(embedding)))),
            sea_orm::Value::Int(Some(embedding.len() as i32)),
            sea_orm::Value::String(Some(Box::new(model.to_string()))),
            sea_orm::Value::Int(Some(row_id)),
        ],
    ))
    .await
    .context("写入整合条目向量失败")?;
    Ok(())
}

/// 存量迁移：把旧 `memory_embeddings` 表里已存在的长期记忆一次性归并进各角色的
/// 共用归档表（`kind='memory'`，沿用原 `memory_id` 与向量）。
///
/// 幂等：目标的档案表里已有该 `memory_id` 时跳过；**全部成功**后才清空旧表数据
/// （保留表结构，兼容 LAN 同步协议与旧端设备）。失败时保留旧表，下次启动重试。
/// 返回迁移的条目数。
pub async fn migrate_memory_embeddings_to_archives(
    db: &DatabaseConnection,
) -> Result<usize, String> {
    #[derive(Debug, FromQueryResult)]
    struct OldRow {
        role_id: i32,
        memory_id: String,
        chunk_index: i32,
        content: String,
        #[sea_orm(column_name = "embedding")]
        embedding: Vec<u8>,
        #[sea_orm(column_name = "dim")]
        dim: i32,
        #[sea_orm(column_name = "model")]
        model: String,
        #[sea_orm(column_name = "created_at")]
        created_at: i64,
    }

    let exists = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' AND name='memory_embeddings'",
        ))
        .await
        .map_err(|e| format!("检查旧版向量表失败: {e}"))?;
    if exists.is_none() {
        return Ok(0);
    }

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT role_id, memory_id, chunk_index, content, embedding, dim, model, created_at \
             FROM memory_embeddings ORDER BY role_id, memory_id, chunk_index",
        ))
        .await
        .map_err(|e| format!("读取旧版向量记忆失败: {e}"))?;
    let mut olds: Vec<OldRow> = Vec::with_capacity(rows.len());
    for row in &rows {
        if let Ok(r) = OldRow::from_query_result(row, "") {
            olds.push(r);
        }
    }
    if olds.is_empty() {
        return Ok(0);
    }

    // 按角色分组，逐角色归并进归档表。
    let mut by_role: HashMap<i32, Vec<&OldRow>> = HashMap::new();
    for old in &olds {
        by_role.entry(old.role_id).or_default().push(old);
    }

    let mut migrated = 0usize;
    for (role_id, group) in by_role {
        let table = ensure_role_archive(db, role_id).await?;

        #[derive(Debug, FromQueryResult)]
        struct MidName {
            #[sea_orm(column_name = "memory_id")]
            memory_id: String,
        }
        let existing = db
            .query_all(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT DISTINCT memory_id FROM \"{table}\" \
                     WHERE kind='memory' AND memory_id IS NOT NULL"
                ),
            ))
            .await
            .map_err(|e| format!("读取归档表既有记忆失败: {e}"))?;
        let mut seen: HashSet<String> = existing
            .iter()
            .filter_map(|row| MidName::from_query_result(row, "").ok())
            .map(|m| m.memory_id)
            .collect();

        for old in group {
            if !seen.insert(old.memory_id.clone()) {
                continue; // 已归并过，幂等跳过
            }
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!(
                    "INSERT INTO \"{table}\" \
                     (kind, memory_id, chunk_index, section, content, embedding, dim, model, created_at) \
                     VALUES ('memory', ?, ?, 'long_term', ?, ?, ?, ?, ?)"
                ),
                vec![
                    sea_orm::Value::String(Some(Box::new(old.memory_id.clone()))),
                    sea_orm::Value::Int(Some(old.chunk_index)),
                    sea_orm::Value::String(Some(Box::new(old.content.clone()))),
                    sea_orm::Value::Bytes(Some(Box::new(old.embedding.clone()))),
                    sea_orm::Value::Int(Some(old.dim)),
                    sea_orm::Value::String(Some(Box::new(old.model.clone()))),
                    sea_orm::Value::BigInt(Some(old.created_at)),
                ],
            ))
            .await
            .map_err(|e| format!("写入归档表失败: {e}"))?;
            migrated += 1;
        }
    }

    // 全部成功才清空旧表数据（保留表结构）。
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "DELETE FROM memory_embeddings",
    ))
    .await
    .map_err(|e| format!("清空旧版向量表失败: {e}"))?;

    tracing::info!("已把 {migrated} 条旧版向量记忆归并进归档表");
    Ok(migrated)
}

/// `memory_embeddings` 表的兜底建表（保留给 LAN 同步协议读写该表时自愈；
/// 长期记忆本身已不再使用它）。
pub async fn ensure_memory_embeddings_table(
    db: &impl ConnectionTrait,
) -> Result<(), sea_orm::DbErr> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        r#"CREATE TABLE IF NOT EXISTS memory_embeddings (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             role_id INTEGER NOT NULL,
             memory_id VARCHAR(64) NOT NULL,
             chunk_index INTEGER NOT NULL,
             content TEXT NOT NULL,
             tags TEXT NULL,
             embedding BLOB NOT NULL,
             dim INTEGER NOT NULL,
             model VARCHAR(128) NOT NULL,
             created_at INTEGER NOT NULL
           )"#,
    ))
    .await?;
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "CREATE INDEX IF NOT EXISTS idx_memory_embeddings_role ON memory_embeddings (role_id)",
    ))
    .await?;
    Ok(())
}

/// 检索/去重时从库读出的一行。
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

/// 浏览用：一条旧版整合条目（`kind='legacy'`，无向量、无 `memory_id`）。
#[derive(Clone, Debug)]
pub struct LegacyRow {
    pub id: i32,
    pub content: String,
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
