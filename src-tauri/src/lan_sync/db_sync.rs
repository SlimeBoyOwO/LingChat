//! 数据库记录级同步。
//!
//! 替代直接传输 `game_database.db` 二进制文件，
//! 使用 SeaORM 实体导出全表数据为 JSON，暂存并在下次启动时导入目标数据库。

use std::path::Path;

use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseBackend, EntityTrait, Statement,
    TransactionTrait,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::db::entities::{
    adventure_unlock, line, line_perception, memory_bank, role, running_script, save,
};

use super::messages::DbRecords;
use super::staging::staging_dir;

/// 暂存文件名。
const DB_RECORDS_FILE: &str = "db_records.json";

// ─── 表操作顺序（按外键依赖） ─────────────────────────────────

/// DELETE 顺序：先删子表再删父表，避免 FK 约束冲突。
const DELETE_ORDER: [&str; 7] = [
    "line_perception",
    "memory_bank",
    "line",
    "running_script",
    "adventure_unlock",
    "save",
    "role",
];

// INSERT 顺序在 apply_staged_db_records 中通过 import_rows! 宏按序调用：
// role → save → running_script → adventure_unlock → line → memory_bank → line_perception

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

/// 导出全部 7 张业务表的数据。
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
        export_table::<adventure_unlock::Entity, adventure_unlock::Model>(
            &db,
            "adventure_unlock",
        )
        .await?;
    let lines = export_table::<line::Entity, line::Model>(&db, "line").await?;
    let memory_banks =
        export_table::<memory_bank::Entity, memory_bank::Model>(&db, "memory_bank").await?;
    let line_perceptions =
        export_table::<line_perception::Entity, line_perception::Model>(&db, "line_perception")
            .await?;

    let total: usize = roles.len()
        + saves.len()
        + running_scripts.len()
        + adventure_unlocks.len()
        + lines.len()
        + memory_banks.len()
        + line_perceptions.len();
    info!("数据库导出完成: {} 条记录", total);

    Ok(DbRecords {
        device_id,
        roles,
        saves,
        running_scripts,
        adventure_unlocks,
        lines,
        memory_banks,
        line_perceptions,
    })
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
            serde_json::to_value(&model)
                .map_err(|e| format!("序列化表 {} 的行失败: {e}", name))
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
    std::fs::create_dir_all(&staging)
        .map_err(|e| format!("创建暂存目录失败: {e}"))?;

    let path = staging.join(DB_RECORDS_FILE);
    let json = serde_json::to_string_pretty(records)
        .map_err(|e| format!("序列化 DB 记录失败: {e}"))?;
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
            }
        },
        Err(e) => {
            warn!("读取暂存 DB 记录失败: {e}");
            None
        }
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

    // 在事务中执行全部操作
    let txn = db
        .begin()
        .await
        .map_err(|e| format!("开始事务失败: {e}"))?;

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
    import_rows!(&txn, &records.saves, save, imported);
    import_rows!(&txn, &records.running_scripts, running_script, imported);
    import_rows!(&txn, &records.adventure_unlocks, adventure_unlock, imported);
    import_rows!(&txn, &records.lines, line, imported);
    import_rows!(&txn, &records.memory_banks, memory_bank, imported);
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
        &s[..s
            .char_indices()
            .nth(max)
            .map(|(i, _)| i)
            .unwrap_or(s.len())]
    }
}
