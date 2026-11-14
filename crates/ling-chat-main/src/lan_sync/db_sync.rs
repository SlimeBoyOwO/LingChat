//! 数据库记录级同步。
//!
//! 替代直接传输 `game_database.db` 二进制文件，
//! 使用 SeaORM 实体导出全表数据为 JSON，暂存并在下次启动时导入目标数据库。

use std::collections::HashSet;
use std::path::Path;

use sea_orm::{
    ColumnTrait, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, EntityTrait,
    QueryFilter, SqlErr, Statement, TransactionTrait,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::db::entities::hotword_weight::normalize_weight;
use crate::db::entities::{
    adventure_unlock, global_hotword, hotword_category, library_hotword, line, line_perception,
    memory_bank, role, role_hotword, running_script, save,
};
use crate::db::managers::hotword_repo::{MAX_CATEGORIES, MAX_CATEGORY_NAME_CHARS};

use super::messages::DbRecords;
use super::staging::staging_dir;

/// 暂存文件名。
const DB_RECORDS_FILE: &str = "db_records.json";

// ─── 表操作顺序（按外键依赖） ─────────────────────────────────

/// DELETE 顺序：先删子表再删父表，避免 FK 约束冲突。
/// 热词表单独列出：旧版 payload 不带热词字段时必须整组跳过，不能当空表清空。
const DELETE_ORDER: [&str; 7] = [
    "line_perception",
    "memory_bank",
    "line",
    "running_script",
    "adventure_unlock",
    "save",
    "role",
];

const HOTWORD_DELETE_ORDER: [&str; 4] = [
    "role_hotword",
    "global_hotword",
    "library_hotword", // 先词条后分类（应用层关系）
    "hotword_category",
];

// INSERT 顺序为父表 → 子表；三张热词表走 import_rows_normalized 归一，分类走 import_categories。

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

/// 热词行归一：`weight` 归一到 `[1, 5] ∪ {50}`，`text` 按本地保存的判据 trim。
/// 不合格时返回原因，调用方丢弃该行。
fn normalize_hotword_row(text: &mut String, weight: &mut i32) -> Result<(), String> {
    *weight = normalize_weight(*weight);
    crate::db::managers::hotword_repo::validate_and_trim(text).map_err(|e| e.to_string())
}

/// 与 `import_rows!` 同形，但每行先过 `normalize` 再 INSERT；闭包返回 `Err` 表示丢弃该行。
///
/// 三张热词表专用：越界 `weight` 会被 DDL 的 CHECK 拒掉整批镜像，故入口归一并
/// 只丢弃校验失败的单行。
async fn import_rows_normalized<E, M, F>(
    db: &impl ConnectionTrait,
    rows: &[Value],
    name: &str,
    normalize: F,
    imported: &mut u64,
) -> Result<(), String>
where
    E: EntityTrait<Model = M>,
    M: serde::de::DeserializeOwned + Into<<E as EntityTrait>::ActiveModel>,
    F: Fn(&mut M) -> Result<(), String>,
{
    if rows.is_empty() {
        return Ok(());
    }
    let models: Vec<M> = serde_json::from_value(Value::Array(rows.to_vec()))
        .map_err(|e| format!("反序列化 {name} 失败: {e}"))?;
    for mut model in models {
        if let Err(reason) = normalize(&mut model) {
            tracing::warn!("[LAN] {name} 有一行未通过校验，已丢弃: {reason}");
            continue;
        }
        let active: <E as EntityTrait>::ActiveModel = model.into();
        match E::insert(active).exec(db).await {
            Ok(_) => *imported += 1,
            // 源端有重复键时只丢这一行，避免整批镜像回滚
            Err(e) if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) => {
                tracing::warn!("[LAN] {name} 有一行与已有记录冲突，已丢弃: {e}");
            },
            Err(e) => return Err(format!("插入 {name} 失败: {e}")),
        }
    }
    Ok(())
}

/// 分类单独导入：trim、丢空、超长、重名只丢该行；条数超上限后丢弃剩余。
///
/// 分类名没有 DDL 约束兜底，不能走不过滤的 `import_rows!`。
async fn import_categories(
    db: &impl ConnectionTrait,
    rows: &[Value],
    imported: &mut u64,
) -> Result<(), String> {
    if rows.is_empty() {
        return Ok(());
    }
    let models: Vec<hotword_category::Model> = serde_json::from_value(Value::Array(rows.to_vec()))
        .map_err(|e| format!("反序列化 hotword_category 失败: {e}"))?;
    let mut names: HashSet<String> = HashSet::new();
    for mut model in models {
        if names.len() >= MAX_CATEGORIES {
            tracing::warn!("[LAN] hotword_category 超过 {MAX_CATEGORIES} 条，已丢弃剩余");
            break;
        }
        let name = model.name.trim().to_string();
        let chars = name.chars().count();
        if name.is_empty() || chars > MAX_CATEGORY_NAME_CHARS {
            tracing::warn!("[LAN] hotword_category 有一行未通过校验，已丢弃: {chars} 字符");
            continue;
        }
        if !names.insert(name.clone()) {
            tracing::warn!("[LAN] hotword_category 有一行重名，已丢弃: {name}");
            continue;
        }
        model.name = name;
        let active: hotword_category::ActiveModel = model.into();
        match hotword_category::Entity::insert(active).exec(db).await {
            Ok(_) => *imported += 1,
            Err(e) if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) => {
                tracing::warn!("[LAN] hotword_category 有一行与已有记录冲突，已丢弃: {e}");
            },
            Err(e) => return Err(format!("插入 hotword_category 失败: {e}")),
        }
    }
    Ok(())
}

// ─── 导出 ─────────────────────────────────────────────────────

/// 导出全部业务表的数据（`DbRecords` 的字段即清单）。
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
    let role_hotwords =
        export_table::<role_hotword::Entity, role_hotword::Model>(&db, "role_hotword").await?;
    let global_hotwords =
        export_table::<global_hotword::Entity, global_hotword::Model>(&db, "global_hotword")
            .await?;
    let hotword_categories =
        export_table::<hotword_category::Entity, hotword_category::Model>(&db, "hotword_category")
            .await?;
    let library_hotwords =
        export_table::<library_hotword::Entity, library_hotword::Model>(&db, "library_hotword")
            .await?;
    let saves = export_table::<save::Entity, save::Model>(&db, "save").await?;
    let running_scripts =
        export_table::<running_script::Entity, running_script::Model>(&db, "running_script")
            .await?;
    let adventure_unlocks =
        export_table::<adventure_unlock::Entity, adventure_unlock::Model>(&db, "adventure_unlock")
            .await?;
    let lines = export_table::<line::Entity, line::Model>(&db, "line").await?;
    let memory_banks =
        export_table::<memory_bank::Entity, memory_bank::Model>(&db, "memory_bank").await?;
    let line_perceptions =
        export_table::<line_perception::Entity, line_perception::Model>(&db, "line_perception")
            .await?;

    let total: usize = roles.len()
        + role_hotwords.len()
        + global_hotwords.len()
        + hotword_categories.len()
        + library_hotwords.len()
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
        role_hotwords: Some(role_hotwords),
        global_hotwords: Some(global_hotwords),
        hotword_categories: Some(hotword_categories),
        library_hotwords: Some(library_hotwords),
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
///
/// 应用失败不上抛：记录 warn、暂存文件改名为 `.bad` 并返回 `Ok(0)`，避免下次启动
/// 重复失败。返回导入的总行数；无暂存记录或应用失败时为 0。
pub async fn apply_staged_db_records(
    db: &DatabaseConnection,
    data_dir: &Path,
) -> Result<u64, String> {
    let records = match load_staged_db_records(data_dir) {
        Some(r) => r,
        None => return Ok(0),
    };

    let staging_path = staging_dir(data_dir).join(DB_RECORDS_FILE);
    let src_id = truncate_str(&records.device_id, 8);
    info!("开始导入数据库记录 (来源: {})...", src_id);

    match apply_records(db, &records).await {
        Ok(imported) => {
            if let Err(e) = std::fs::remove_file(&staging_path) {
                warn!("清理暂存 DB 记录文件失败: {e}");
            }
            info!("数据库记录导入完成: {} 行", imported);
            Ok(imported)
        },
        Err(e) => {
            warn!("应用暂存 DB 记录失败，跳过本次导入: {e}");
            let bad_path = staging_path.with_extension("json.bad");
            if let Err(e) = std::fs::rename(&staging_path, &bad_path) {
                warn!("暂存 DB 记录文件改名失败: {e}");
            } else {
                info!("已保留失败记录: {:?}", bad_path);
            }
            Ok(0)
        },
    }
}

/// 在单个事务中应用记录：按依赖顺序 DELETE 全部行、再重新 INSERT。
///
/// 事务内 `PRAGMA foreign_keys = OFF` 不生效，导入期间外键仍开启，因此
/// DELETE_ORDER 与 INSERT 顺序是硬约束。失败即回滚。
async fn apply_records(db: &DatabaseConnection, records: &DbRecords) -> Result<u64, String> {
    // 在事务中执行全部操作
    let txn = db.begin().await.map_err(|e| format!("开始事务失败: {e}"))?;
    txn.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        "PRAGMA foreign_keys = OFF",
    ))
    .await
    .map_err(|e| format!("禁用外键检查失败: {e}"))?;

    let mut imported: u64 = 0;

    // 旧版 payload 不带热词字段：整组跳过，否则会把本机热词当空表清掉
    let hotwords_included = records.role_hotwords.is_some()
        || records.global_hotwords.is_some()
        || records.hotword_categories.is_some()
        || records.library_hotwords.is_some();

    // 玩家角色是本地身份卡的投影，不参与整表镜像；先建立远端 id → 本地 id 映射，
    // 供存档与台词里的玩家角色引用翻译。
    let local_players = role::Entity::find()
        .filter(role::Column::RoleType.eq(role::RoleType::Player))
        .all(&txn)
        .await
        .map_err(|e| format!("读取本地玩家角色失败: {e}"))?;
    let local_by_identity: std::collections::HashMap<String, i32> = local_players
        .iter()
        .filter_map(|r| r.identity_id.clone().map(|id| (id, r.id)))
        .collect();
    let mut remote_player_map: std::collections::HashMap<i64, i32> =
        std::collections::HashMap::new();
    let mut remote_player_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for row in &records.roles {
        if row.get("role_type").and_then(Value::as_str) != Some("player") {
            continue;
        }
        let Some(remote_id) = row.get("id").and_then(Value::as_i64) else {
            continue;
        };
        remote_player_ids.insert(remote_id);
        let Some(identity_id) = row.get("identity_id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(local_id) = local_by_identity.get(identity_id) {
            remote_player_map.insert(remote_id, *local_id);
        }
    }
    let mut saves = records.saves.clone();
    for row in &mut saves {
        translate_player_ref(
            row,
            "player_role_id",
            &remote_player_map,
            &remote_player_ids,
            Value::Null,
        );
    }
    let mut lines = records.lines.clone();
    for row in &mut lines {
        translate_player_ref(
            row,
            "sender_role_id",
            &remote_player_map,
            &remote_player_ids,
            Value::from(0),
        );
    }
    let remote_roles: Vec<Value> = records
        .roles
        .iter()
        .filter(|row| row.get("role_type").and_then(Value::as_str) != Some("player"))
        .cloned()
        .collect();

    // ─── DELETE 全部行（子表 → 父表顺序） ────────────────
    for table in DELETE_ORDER {
        let count = if table == "role" {
            txn.execute(Statement::from_string(
                DatabaseBackend::Sqlite,
                "DELETE FROM role WHERE role_type <> 'player'".to_string(),
            ))
            .await
            .map_err(|e| format!("清空非玩家角色失败: {e}"))?
            .rows_affected()
        } else {
            delete_all(&txn, table).await?
        };
        if count > 0 {
            info!("已清除 {}: {} 行", table, count);
        }
    }
    if hotwords_included {
        for table in HOTWORD_DELETE_ORDER {
            let count = delete_all(&txn, table).await?;
            if count > 0 {
                info!("已清除 {}: {} 行", table, count);
            }
        }
    }

    // ─── INSERT 全部行（父表 → 子表顺序） ────────────────
    import_rows!(&txn, &remote_roles, role, imported);
    import_rows_normalized::<role_hotword::Entity, _, _>(
        &txn,
        records.role_hotwords.as_deref().unwrap_or(&[]),
        "role_hotword",
        |m| normalize_hotword_row(&mut m.text, &mut m.weight),
        &mut imported,
    )
    .await?;
    import_rows_normalized::<global_hotword::Entity, _, _>(
        &txn,
        records.global_hotwords.as_deref().unwrap_or(&[]),
        "global_hotword",
        |m| normalize_hotword_row(&mut m.text, &mut m.weight),
        &mut imported,
    )
    .await?;
    import_categories(
        &txn,
        records.hotword_categories.as_deref().unwrap_or(&[]),
        &mut imported,
    )
    .await?;
    import_rows_normalized::<library_hotword::Entity, _, _>(
        &txn,
        records.library_hotwords.as_deref().unwrap_or(&[]),
        "library_hotword",
        |m| normalize_hotword_row(&mut m.text, &mut m.weight),
        &mut imported,
    )
    .await?;
    import_rows!(&txn, &saves, save, imported);
    import_rows!(&txn, &records.running_scripts, running_script, imported);
    import_rows!(&txn, &records.adventure_unlocks, adventure_unlock, imported);
    import_rows!(&txn, &lines, line, imported);
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

    Ok(imported)
}

/// 把远端玩家角色 id 翻译成本地 id；远端玩家行不存在时写 fallback。
fn translate_player_ref(
    row: &mut Value,
    field: &str,
    map: &std::collections::HashMap<i64, i32>,
    remote_player_ids: &std::collections::HashSet<i64>,
    fallback: Value,
) {
    let Some(obj) = row.as_object_mut() else {
        return;
    };
    let Some(remote_id) = obj.get(field).and_then(Value::as_i64) else {
        return;
    };
    if !remote_player_ids.contains(&remote_id) {
        return;
    }
    let local = map
        .get(&remote_id)
        .map(|id| Value::from(*id))
        .unwrap_or(fallback);
    obj.insert(field.to_string(), local);
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
