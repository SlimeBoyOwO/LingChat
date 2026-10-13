//! 数据库记录级同步。
//!
//! 替代直接传输 `game_database.db` 二进制文件，
//! 使用 SeaORM 实体导出全表数据为 JSON，暂存并在下次启动时导入目标数据库。

use std::path::Path;

use base64::Engine;
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, EntityTrait, Set, Statement,
    TransactionTrait,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::ai_service::memory::store;
use crate::db::entities::{
    adventure_unlock, line, line_perception, memory_embedding, role, running_script, save,
};

use super::messages::DbRecords;
use super::staging::staging_dir;

/// 暂存文件名。
const DB_RECORDS_FILE: &str = "db_records.json";

/// base64 标准字母表（记忆向量在 JSON 里的传输编码）。
const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

// ─── 表操作顺序（按外键依赖） ─────────────────────────────────

/// DELETE 顺序：先删子表再删父表，避免 FK 约束冲突。
/// `memory_bank` 是旧版记忆链路遗留表（表结构保留、不再读写），一并清掉避免残留。
///
/// 注意：`memory_embeddings` **不在此列**——它是后加的表，旧对端发来的记录里没有
/// 这个字段，无条件清空会把本地长期记忆抹掉。它在导入时按记录里是否有该字段
/// 决定是否整表镜像（见 [`apply_staged_db_records`]）。
const DELETE_ORDER: [&str; 7] = [
    "line_perception",
    "memory_bank",
    "line",
    "running_script",
    "adventure_unlock",
    "save",
    "role",
];

// INSERT 顺序在 apply_staged_db_records 中按序调用：
// role → memory_embeddings → save → running_script → adventure_unlock → line → line_perception

/// 导入宏：将 JSON 数组反序列化为 Entity Model，转为 ActiveModel 逐行 INSERT。
///
/// 保留原始主键 ID，实现源端数据镜像。
macro_rules! import_rows {
    ($db:expr, $rows:expr, $entity:ident, $count:expr) => {{
        if !$rows.is_empty() {
            let models: Vec<$entity::Model> = serde_json::from_value(Value::Array($rows.to_vec()))
                .map_err(|e| format!("反序列化 {} 失败: {e}", stringify!($entity)))?;
            for model in models {
                let active: $entity::ActiveModel = model.into();
                $entity::Entity::insert(active)
                    .exec($db)
                    .await
                    .map_err(|e| format!("插入 {} 失败: {e}", stringify!($entity)))?;
                $count += 1;
            }
        }
    }};
}

// ─── 导出 ─────────────────────────────────────────────────────

/// 导出全部业务表的数据（memory_bank 旧记忆表不再导出，memory_embeddings 长期记忆随行导出；
/// RAG 检索行 attribute="Memory" 按行过滤，不跨端同步）。
///
/// 通过独立只读连接打开数据库（`mode=ro`），不阻塞主应用的读写连接。
pub async fn export_all_records(data_dir: &Path) -> Result<DbRecords, String> {
    let db_path = data_dir.join("game_database.db");

    if !db_path.exists() {
        return Err("数据库文件不存在，无法导出".to_string());
    }

    let db_url = format!("sqlite:{}?mode=ro", db_path.display());
    let db = Database::connect(&db_url)
        .await
        .map_err(|e| format!("连接数据库失败(只读): {e}"))?;

    let device_id = read_device_id(data_dir).unwrap_or_default();

    info!("开始导出数据库记录...");

    let roles = export_table::<role::Entity, role::Model>(&db, "role").await?;
    let saves = export_table::<save::Entity, save::Model>(&db, "save").await?;
    let running_scripts =
        export_table::<running_script::Entity, running_script::Model>(&db, "running_script")
            .await?;
    let adventure_unlocks =
        export_table::<adventure_unlock::Entity, adventure_unlock::Model>(&db, "adventure_unlock")
            .await?;
    let lines = export_table::<line::Entity, line::Model>(&db, "line").await?;
    let line_perceptions =
        export_table::<line_perception::Entity, line_perception::Model>(&db, "line_perception")
            .await?;
    // RAG 检索行（attribute="Memory"）是本机当轮生成的临时上下文，不参与同步：
    // 对端下一条用户消息会自行检索重建，跨端镜像没有价值；而且旧对端的
    // LineAttribute 枚举没有这个变体，整批 line 反序列化会直接失败。
    let mut skip_line_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let lines: Vec<Value> = lines
        .into_iter()
        .filter(|row| {
            if row.get("attribute").and_then(Value::as_str) == Some("Memory") {
                if let Some(id) = row.get("id").and_then(Value::as_i64) {
                    skip_line_ids.insert(id);
                }
                false
            } else {
                true
            }
        })
        .collect();
    // 连带过滤这些行的感知记录，避免对端落孤儿行
    let line_perceptions: Vec<Value> = line_perceptions
        .into_iter()
        .filter(|row| {
            !row.get("line_id")
                .and_then(Value::as_i64)
                .is_some_and(|id| skip_line_ids.contains(&id))
        })
        .collect();
    let memory_embeddings = export_memory_embeddings(&db).await?;

    let total: usize = roles.len()
        + saves.len()
        + running_scripts.len()
        + adventure_unlocks.len()
        + lines.len()
        + line_perceptions.len()
        + memory_embeddings.len();
    info!("数据库导出完成: {} 条记录", total);

    Ok(DbRecords {
        device_id,
        roles,
        saves,
        running_scripts,
        adventure_unlocks,
        lines,
        memory_banks: Vec::new(),
        line_perceptions,
        memory_embeddings: Some(memory_embeddings),
    })
}

/// LAN 同步用的 `memory_embeddings` 行：与实体同构，仅 `embedding` 换成 base64。
///
/// 向量原样走 JSON 会变成几千个数字的数组（每条记忆 10KB+ 的文本膨胀），
/// base64 后体积约为裸字节的 4/3，且导入端解析零歧义。
#[derive(serde::Serialize, serde::Deserialize)]
struct MemoryEmbeddingRecord {
    id: i32,
    role_id: i32,
    memory_id: String,
    chunk_index: i32,
    content: String,
    tags: Option<String>,
    embedding: String,
    dim: i32,
    model: String,
    created_at: i64,
}

/// 导出长期记忆表（embedding → base64）。
async fn export_memory_embeddings(db: &DatabaseConnection) -> Result<Vec<Value>, String> {
    // 幂等兜底：旧库可能缺这张表（长期记忆已迁入归档表，此表仅 LAN 同步协议保留读写）。
    store::ensure_memory_embeddings_table(db)
        .await
        .map_err(|e| format!("确保 memory_embeddings 表存在失败: {e}"))?;
    let rows = memory_embedding::Entity::find()
        .all(db)
        .await
        .map_err(|e| format!("查询表 memory_embeddings 失败: {e}"))?;

    rows.into_iter()
        .map(|row| {
            serde_json::to_value(MemoryEmbeddingRecord {
                id: row.id,
                role_id: row.role_id,
                memory_id: row.memory_id,
                chunk_index: row.chunk_index,
                content: row.content,
                tags: row.tags,
                embedding: B64.encode(&row.embedding),
                dim: row.dim,
                model: row.model,
                created_at: row.created_at,
            })
            .map_err(|e| format!("序列化表 memory_embeddings 的行失败: {e}"))
        })
        .collect()
}

/// 导入长期记忆表（base64 → 向量），保留原始 ID。
async fn import_memory_embeddings(
    db: &impl ConnectionTrait,
    rows: &[Value],
    count: &mut u64,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    let records: Vec<MemoryEmbeddingRecord> =
        serde_json::from_value(Value::Array(rows.to_vec()))
            .map_err(|e| format!("反序列化 memory_embeddings 失败: {e}"))?;
    for record in records {
        let embedding = B64
            .decode(record.embedding.as_bytes())
            .map_err(|e| format!("memory_embeddings 向量 base64 解码失败: {e}"))?;
        let model = memory_embedding::ActiveModel {
            id: Set(record.id),
            role_id: Set(record.role_id),
            memory_id: Set(record.memory_id),
            chunk_index: Set(record.chunk_index),
            content: Set(record.content),
            tags: Set(record.tags),
            embedding: Set(embedding),
            dim: Set(record.dim),
            model: Set(record.model),
            created_at: Set(record.created_at),
        };
        memory_embedding::Entity::insert(model)
            .exec(db)
            .await
            .map_err(|e| format!("插入 memory_embeddings 失败: {e}"))?;
        *count += 1;
    }
    Ok(())
}

/// 导出单张表的所有行，序列化为 `Vec<serde_json::Value>`。
async fn export_table<E, M>(db: &DatabaseConnection, name: &str) -> Result<Vec<Value>, String>
where
    E: EntityTrait<Model = M>,
    M: serde::Serialize,
{
    let rows = E::find()
        .all(db)
        .await
        .map_err(|e| format!("查询表 {} 失败: {e}", name))?;

    rows.into_iter()
        .map(|model| {
            serde_json::to_value(&model).map_err(|e| format!("序列化表 {} 的行失败: {e}", name))
        })
        .collect()
}

/// 从磁盘读取设备 ID（仅用于导出标识来源）。
fn read_device_id(data_dir: &Path) -> Option<String> {
    let path = data_dir.join(".lan_sync_device.json");
    let content = std::fs::read_to_string(&path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v.get("deviceId") // camelCase（DeviceIdentity 序列化格式）
        .or_else(|| v.get("device_id")) // snake_case 兼容
        .and_then(|id| id.as_str())
        .map(String::from)
}

// ─── 暂存 ─────────────────────────────────────────────────────

/// 将数据库记录暂存到 `.lan_sync_staging/db_records.json`。
pub fn stage_db_records(data_dir: &Path, records: &DbRecords) -> Result<(), String> {
    let staging = staging_dir(data_dir);
    std::fs::create_dir_all(&staging).map_err(|e| format!("创建暂存目录失败: {e}"))?;

    let path = staging.join(DB_RECORDS_FILE);
    let json =
        serde_json::to_string_pretty(records).map_err(|e| format!("序列化 DB 记录失败: {e}"))?;
    std::fs::write(&path, json).map_err(|e| format!("写入暂存 DB 记录失败: {e}"))?;

    info!("数据库记录已暂存: {:?}", path);
    Ok(())
}

/// 从暂存目录加载数据库记录（如果存在）。
pub fn load_staged_db_records(data_dir: &Path) -> Option<DbRecords> {
    let path = staging_dir(data_dir).join(DB_RECORDS_FILE);
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(&path) {
        Ok(json) => match serde_json::from_str::<DbRecords>(&json) {
            Ok(records) => Some(records),
            Err(e) => {
                warn!("解析暂存 DB 记录失败: {e}");
                None
            },
        },
        Err(e) => {
            warn!("读取暂存 DB 记录失败: {e}");
            None
        },
    }
}

// ─── 导入 ─────────────────────────────────────────────────────

/// 将暂存的数据库记录应用到主数据库。
///
/// 在 `db::init_db()` 调用之后执行（此时表结构已就绪）。
/// 通过 `PRAGMA foreign_keys = OFF` 禁用外键检查，
/// 按依赖顺序 DELETE 全部行后重新 INSERT。
///
/// 返回导入的总行数，如果无暂存记录则返回 0。
pub async fn apply_staged_db_records(
    db: &DatabaseConnection,
    data_dir: &Path,
) -> Result<u64, String> {
    let records = match load_staged_db_records(data_dir) {
        Some(r) => r,
        None => return Ok(0),
    };

    let src_id = truncate_str(&records.device_id, 8);
    info!("开始导入数据库记录 (来源: {})...", src_id);

    // 幂等兜底：旧库可能缺 memory_embeddings 表（长期记忆迁入归档表后，该表仅对端
    // 记录里带该字段时才涉及），先确保表存在再进事务，避免 delete_all 报 no such table。
    store::ensure_memory_embeddings_table(db)
        .await
        .map_err(|e| format!("确保 memory_embeddings 表存在失败: {e}"))?;

    // 在事务中执行全部操作
    let txn = db.begin().await.map_err(|e| format!("开始事务失败: {e}"))?;

    // 禁用外键检查
    txn.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = OFF",
    ))
    .await
    .map_err(|e| format!("禁用外键检查失败: {e}"))?;

    let mut imported: u64 = 0;

    // ─── DELETE 全部行（子表 → 父表顺序） ────────────────
    for table in DELETE_ORDER {
        let count = delete_all(&txn, table).await?;
        if count > 0 {
            info!("已清除 {}: {} 行", table, count);
        }
    }

    // ─── INSERT 全部行（父表 → 子表顺序） ────────────────
    import_rows!(&txn, &records.roles, role, imported);
    // 长期记忆只在对端记录里带该字段时整表镜像；旧对端没有这个概念，保留本地记忆。
    if let Some(memory_embeddings) = &records.memory_embeddings {
        let cleared = delete_all(&txn, "memory_embeddings").await?;
        if cleared > 0 {
            info!("已清除 memory_embeddings: {} 行", cleared);
        }
        import_memory_embeddings(&txn, memory_embeddings, &mut imported).await?;
    }
    import_rows!(&txn, &records.saves, save, imported);
    import_rows!(&txn, &records.running_scripts, running_script, imported);
    import_rows!(&txn, &records.adventure_unlocks, adventure_unlock, imported);
    import_rows!(&txn, &records.lines, line, imported);
    import_rows!(&txn, &records.line_perceptions, line_perception, imported);

    // 重新启用外键检查
    txn.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = ON",
    ))
    .await
    .map_err(|e| format!("启用外键检查失败: {e}"))?;

    txn.commit()
        .await
        .map_err(|e| format!("提交事务失败: {e}"))?;

    // 清理暂存文件
    let staging_path = staging_dir(data_dir).join(DB_RECORDS_FILE);
    if let Err(e) = std::fs::remove_file(&staging_path) {
        warn!("清理暂存 DB 记录文件失败: {e}");
    }

    info!("数据库记录导入完成: {} 行", imported);
    Ok(imported)
}

/// 删除表中的全部行。
async fn delete_all(db: &impl ConnectionTrait, table: &str) -> Result<u64, String> {
    let sql = format!("DELETE FROM {}", table);
    let result = db
        .execute(Statement::from_string(DatabaseBackend::Sqlite, sql))
        .await
        .map_err(|e| format!("清空表 {} 失败: {e}", table))?;
    Ok(result.rows_affected())
}

fn truncate_str(s: &str, max: usize) -> &str {
    let len = s.chars().count();
    if len <= max {
        s
    } else {
        &s[..s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len())]
    }
}
