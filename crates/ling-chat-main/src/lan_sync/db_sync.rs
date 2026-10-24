//! 数据库记录级同步。
//!
//! 替代直接传输 `game_database.db` 二进制文件，
//! 使用 SeaORM 实体导出全表数据为 JSON，暂存并在下次启动时导入目标数据库。

use std::path::Path;

use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, EntityTrait, Statement,
    TransactionTrait,
};
use serde_json::Value;
use tracing::{info, warn};

use crate::db::entities::hotword_weight::normalize_weight;
use crate::db::entities::{
    adventure_unlock, global_hotword, hotword_category, library_hotword, line, line_perception,
    memory_bank, role, role_hotword, running_script, save,
};

use super::messages::DbRecords;
use super::staging::staging_dir;

/// 暂存文件名。
const DB_RECORDS_FILE: &str = "db_records.json";

// ─── 表操作顺序（按外键依赖） ─────────────────────────────────

/// DELETE 顺序：先删子表再删父表，避免 FK 约束冲突。
/// 三张 global / 词库表无外键，位置只为可读性（紧跟角色热词）。
const DELETE_ORDER: [&str; 11] = [
    "line_perception",
    "memory_bank",
    "line",
    "running_script",
    "adventure_unlock",
    "save",
    "role_hotword",
    "global_hotword",
    "library_hotword", // 先词条后分类（应用层关系）
    "hotword_category",
    "role",
];

// INSERT 顺序在 apply_staged_db_records 中按序调用：
// role → role_hotword → global_hotword
// → hotword_category → library_hotword
// → save → running_script → adventure_unlock → line → memory_bank → line_perception
// 其中三张热词表走 import_rows_normalized（入口归一），其余走 import_rows! 宏。

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

/// 热词行归一：`weight` 归一到 `[1, 5] ∪ {50}`，`text` 按本地保存同一判据处理，
/// 不合格返回原因（该行丢弃）。`text` 是归并/唯一键，未 `trim` 会拆成两个 key。
fn normalize_hotword_row(text: &mut String, weight: &mut i32) -> Result<(), String> {
    *weight = normalize_weight(*weight);
    crate::db::managers::hotword_repo::validate_and_trim(text).map_err(|e| e.to_string())
}

/// 与 `import_rows!` 同形，但每行先过 `normalize` 再 INSERT；闭包返回 `Err` 表示**丢弃该行**。
///
/// 三张热词表专用：对端 payload 里的脏值必须在此归一（与本地保存同一口径）——
/// 越界 `weight` 会让 DDL 的 CHECK 拒掉**整批**镜像，是用户看不见的静默失效。
/// 但**单行**未通过校验（空词、超长）只丢该行：同步是整台设备的镜像，
/// 一行坏数据不该让整批镜像回滚。
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
        E::insert(model.into())
            .exec(db)
            .await
            .map_err(|e| format!("插入 {name} 失败: {e}"))?;
        *imported += 1;
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
        role_hotwords,
        global_hotwords,
        hotword_categories,
        library_hotwords,
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
/// 应用阶段的失败不上抛（否则启动 panic，而暂存文件因未提交而残留，
/// 下次启动重蹈覆辙）：记录 warn、把暂存文件改名为 `.bad`，并返回 `Ok(0)`。
///
/// 返回导入的总行数，如果无暂存记录或应用失败则返回 0。
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
/// 注意：下方 `PRAGMA foreign_keys = OFF` 在事务内是 no-op，
/// **导入期间外键实际是开启的**，所以 DELETE_ORDER 与 INSERT 的先后顺序是硬约束。
/// 失败即回滚（事务随 drop 丢弃）。
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

    // ─── DELETE 全部行（子表 → 父表顺序） ────────────────
    for table in DELETE_ORDER {
        let count = delete_all(&txn, table).await?;
        if count > 0 {
            info!("已清除 {}: {} 行", table, count);
        }
    }

    // ─── INSERT 全部行（父表 → 子表顺序） ────────────────
    import_rows!(&txn, &records.roles, role, imported);
    import_rows_normalized::<role_hotword::Entity, _, _>(
        &txn,
        &records.role_hotwords,
        "role_hotword",
        |m| normalize_hotword_row(&mut m.text, &mut m.weight),
        &mut imported,
    )
    .await?;
    import_rows_normalized::<global_hotword::Entity, _, _>(
        &txn,
        &records.global_hotwords,
        "global_hotword",
        |m| normalize_hotword_row(&mut m.text, &mut m.weight),
        &mut imported,
    )
    .await?;
    // 分类名没有 CHECK 约束，按普通宏原样导入
    import_rows!(
        &txn,
        &records.hotword_categories,
        hotword_category,
        imported
    );
    import_rows_normalized::<library_hotword::Entity, _, _>(
        &txn,
        &records.library_hotwords,
        "library_hotword",
        |m| normalize_hotword_row(&mut m.text, &mut m.weight),
        &mut imported,
    )
    .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, Set};

    /// 畸形 payload 在应用阶段失败时不应上抛 `Err`（否则整段同步被跳过），
    /// 且暂存文件应改名为 `.bad`，避免下次启动重复失败。
    #[tokio::test]
    async fn malformed_payload_does_not_fail_startup() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let data_dir = tmp.path();

        let db = crate::db::init_db(data_dir)
            .await
            .expect("初始化数据库失败");

        // roles 中一行 name 类型非法：行本身只是 Value，能通过 load_staged_db_records，
        // 但在 import_rows! 反序列化为 role::Model 时失败（应用阶段）。
        let records = DbRecords {
            device_id: "test-device".to_string(),
            roles: vec![serde_json::json!({ "id": 7, "name": 123 })],
            saves: vec![],
            running_scripts: vec![],
            adventure_unlocks: vec![],
            lines: vec![],
            memory_banks: vec![],
            line_perceptions: vec![],
            role_hotwords: vec![],
            global_hotwords: vec![],
            hotword_categories: vec![],
            library_hotwords: vec![],
        };
        stage_db_records(data_dir, &records).expect("暂存失败");
        assert!(
            load_staged_db_records(data_dir).is_some(),
            "用例前提：payload 应能通过加载阶段"
        );

        let result = apply_staged_db_records(&db, data_dir).await;
        let staging = staging_dir(data_dir);

        assert_eq!(result, Ok(0), "畸形 payload 不应上抛错误");
        assert!(
            !staging.join(DB_RECORDS_FILE).exists(),
            "原暂存文件应已被移走"
        );
        assert!(
            staging.join("db_records.json.bad").exists(),
            "应生成 .bad 文件"
        );
    }

    /// 导出→导入后热词必须与源端一致。
    #[tokio::test]
    async fn mirror_includes_hotwords() {
        let src = tempfile::tempdir().expect("创建临时目录失败");
        let src_db = crate::db::init_db(src.path())
            .await
            .expect("初始化源数据库失败");

        role::ActiveModel {
            id: Set(7),
            name: Set("甲".to_string()),
            role_type: Set(role::RoleType::Main),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入角色失败");
        role_hotword::ActiveModel {
            role_id: Set(7),
            text: Set("张三".to_string()),
            weight: Set(5),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入热词失败");

        let records = export_all_records(src.path()).await.expect("导出失败");
        assert_eq!(records.role_hotwords.len(), 1, "导出应包含热词");

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        // 目标端预置另一个角色的热词，验证镜像会先清空该表
        role_hotword::ActiveModel {
            role_id: Set(99),
            text: Set("旧词".to_string()),
            weight: Set(4),
            ..Default::default()
        }
        .insert(&dst_db)
        .await
        .expect("预置热词失败");

        stage_db_records(dst.path(), &records).expect("暂存失败");

        let imported = apply_staged_db_records(&dst_db, dst.path())
            .await
            .expect("导入失败");
        assert!(imported >= 2, "至少应导入角色 + 热词，实际 {imported}");

        let hotwords = role_hotword::Entity::find()
            .all(&dst_db)
            .await
            .expect("查询热词失败");
        assert_eq!(hotwords.len(), 1, "镜像应先清空旧热词，再导入源端热词");
        assert_eq!(hotwords[0].role_id, 7);
        assert_eq!((hotwords[0].text.as_str(), hotwords[0].weight), ("张三", 5));
    }

    /// 导出→导入后全局热词必须与源端一致。
    #[tokio::test]
    async fn mirror_includes_global_hotwords() {
        let src = tempfile::tempdir().expect("创建临时目录失败");
        let src_db = crate::db::init_db(src.path())
            .await
            .expect("初始化源数据库失败");
        global_hotword::ActiveModel {
            text: Set("全局词".to_string()),
            weight: Set(5),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入全局热词失败");

        let records = export_all_records(src.path()).await.expect("导出失败");
        assert_eq!(records.global_hotwords.len(), 1, "导出应包含全局热词");

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        // 目标端预置旧数据，验证镜像会先清空该表
        global_hotword::ActiveModel {
            text: Set("旧词".to_string()),
            weight: Set(4),
            ..Default::default()
        }
        .insert(&dst_db)
        .await
        .expect("预置全局热词失败");

        stage_db_records(dst.path(), &records).expect("暂存失败");
        let imported = apply_staged_db_records(&dst_db, dst.path())
            .await
            .expect("导入失败");
        assert!(imported >= 1, "至少应导入全局热词，实际 {imported}");

        let hotwords = global_hotword::Entity::find()
            .all(&dst_db)
            .await
            .expect("查询全局热词失败");
        assert_eq!(hotwords.len(), 1, "镜像应先清空旧全局热词，再导入源端数据");
        assert_eq!(
            (hotwords[0].text.as_str(), hotwords[0].weight),
            ("全局词", 5)
        );
    }

    /// 导出→导入后词库必须与源端一致（含未分类与分类关系）。
    #[tokio::test]
    async fn mirror_includes_hotword_library() {
        let src = tempfile::tempdir().expect("创建临时目录失败");
        let src_db = crate::db::init_db(src.path())
            .await
            .expect("初始化源数据库失败");

        let cat = hotword_category::ActiveModel {
            name: Set("人物".to_string()),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入分类失败");
        library_hotword::ActiveModel {
            category_id: Set(Some(cat.id)),
            text: Set("张三".to_string()),
            weight: Set(9),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入词条失败");
        library_hotword::ActiveModel {
            category_id: Set(None),
            text: Set("通用".to_string()),
            weight: Set(4),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入未分类词条失败");

        let records = export_all_records(src.path()).await.expect("导出失败");
        assert_eq!(records.hotword_categories.len(), 1, "导出应含分类");
        assert_eq!(records.library_hotwords.len(), 2, "导出应含词条");

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        // 目标端预置旧数据，验证镜像先清空这两张表
        hotword_category::ActiveModel {
            name: Set("旧分类".to_string()),
            ..Default::default()
        }
        .insert(&dst_db)
        .await
        .expect("预置分类失败");

        stage_db_records(dst.path(), &records).expect("暂存失败");
        apply_staged_db_records(&dst_db, dst.path())
            .await
            .expect("导入失败");

        let cats = hotword_category::Entity::find()
            .all(&dst_db)
            .await
            .expect("查询分类失败");
        assert_eq!(cats.len(), 1, "镜像应先清空旧分类");
        assert_eq!(cats[0].name, "人物");

        let words = library_hotword::Entity::find()
            .all(&dst_db)
            .await
            .expect("查询词条失败");
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].category_id, Some(cats[0].id), "分类关系随镜像保持");
        assert_eq!(words[1].category_id, None, "未分类仍是 NULL");
    }
}
