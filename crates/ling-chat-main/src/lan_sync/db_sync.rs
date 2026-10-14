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

/// 与 `import_rows!` 同形，但每行先过 `normalize` 再 INSERT；闭包返回 `false` 表示**丢弃该行**。
///
/// 三张热词表专用：对端 payload 里的脏值必须在此归一（spec §3.2 各写入入口同一口径）——
/// 越界 `weight` 会让 DDL 的 CHECK 拒掉**整批**镜像，是用户看不见的静默失效。
/// 长度判据同样在此执行（与本地保存同源），但超长行只丢该行、不让整批失败。
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
    F: Fn(&mut M) -> bool,
{
    if rows.is_empty() {
        return Ok(());
    }
    let models: Vec<M> = serde_json::from_value(Value::Array(rows.to_vec()))
        .map_err(|e| format!("反序列化 {name} 失败: {e}"))?;
    for mut model in models {
        if !normalize(&mut model) {
            tracing::warn!("[LAN] {name} 有一行未通过校验，已丢弃");
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

/// 导出全部 11 张业务表的数据。
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

    // 事务内为 no-op，导入期间外键仍然开启
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
        |m| {
            m.weight = normalize_weight(m.weight);
            // 长度判据与本地保存同源（`validate_text_len`）：否则「本地被拒的值可以
            // 从对端镜像进来」。超长只丢这一行——同步是整台设备的镜像，
            // 一行坏数据不该回滚 11 张表。
            //
            // ⚠️ **必须先 trim、再判空、再校验、最后把 trim 后的值写回**，缺一不可：
            // `validate_text_len` 只判长度，不管空白（它是对给定字符串的谓词）。若直接
            // 拿原始 `m.text` 去判，会产生三处与本地不一致——① 词首尾带空格的合法词在
            // 本地能存、镜像却被丢（静默数据丢失）；② 纯空白串 `is_ascii()` 且 0 片段，
            // 会被放行入库，造出本地永远产不出的空词条；③ 未 trim 的文本以其**原样**
            // 入库，而 `text` 正是归并/唯一键，同一个词会出现两个不同的 key，把同步
            // 本该保住的去重拆开。
            let text = m.text.trim();
            if text.is_empty() {
                tracing::warn!("[LAN] 热词为空，已丢弃");
                return false;
            }
            if let Err(e) = crate::db::managers::hotword_repo::validate_text_len(text) {
                tracing::warn!("[LAN] 热词未通过长度校验，已丢弃: {e}");
                return false;
            }
            m.text = text.to_string();
            true
        },
        &mut imported,
    )
    .await?;
    import_rows_normalized::<global_hotword::Entity, _, _>(
        &txn,
        &records.global_hotwords,
        "global_hotword",
        |m| {
            m.weight = normalize_weight(m.weight);
            // 长度判据与本地保存同源（`validate_text_len`）：否则「本地被拒的值可以
            // 从对端镜像进来」。超长只丢这一行——同步是整台设备的镜像，
            // 一行坏数据不该回滚 11 张表。
            //
            // ⚠️ **必须先 trim、再判空、再校验、最后把 trim 后的值写回**，缺一不可：
            // `validate_text_len` 只判长度，不管空白（它是对给定字符串的谓词）。若直接
            // 拿原始 `m.text` 去判，会产生三处与本地不一致——① 词首尾带空格的合法词在
            // 本地能存、镜像却被丢（静默数据丢失）；② 纯空白串 `is_ascii()` 且 0 片段，
            // 会被放行入库，造出本地永远产不出的空词条；③ 未 trim 的文本以其**原样**
            // 入库，而 `text` 正是归并/唯一键，同一个词会出现两个不同的 key，把同步
            // 本该保住的去重拆开。
            let text = m.text.trim();
            if text.is_empty() {
                tracing::warn!("[LAN] 热词为空，已丢弃");
                return false;
            }
            if let Err(e) = crate::db::managers::hotword_repo::validate_text_len(text) {
                tracing::warn!("[LAN] 热词未通过长度校验，已丢弃: {e}");
                return false;
            }
            m.text = text.to_string();
            true
        },
        &mut imported,
    )
    .await?;
    // 分类名无 DDL 约束，与 role_hotword.text 镜像不 trim 同理，走普通宏
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
        |m| {
            m.weight = normalize_weight(m.weight);
            // 长度判据与本地保存同源（`validate_text_len`）：否则「本地被拒的值可以
            // 从对端镜像进来」。超长只丢这一行——同步是整台设备的镜像，
            // 一行坏数据不该回滚 11 张表。
            //
            // ⚠️ **必须先 trim、再判空、再校验、最后把 trim 后的值写回**，缺一不可：
            // `validate_text_len` 只判长度，不管空白（它是对给定字符串的谓词）。若直接
            // 拿原始 `m.text` 去判，会产生三处与本地不一致——① 词首尾带空格的合法词在
            // 本地能存、镜像却被丢（静默数据丢失）；② 纯空白串 `is_ascii()` 且 0 片段，
            // 会被放行入库，造出本地永远产不出的空词条；③ 未 trim 的文本以其**原样**
            // 入库，而 `text` 正是归并/唯一键，同一个词会出现两个不同的 key，把同步
            // 本该保住的去重拆开。
            let text = m.text.trim();
            if text.is_empty() {
                tracing::warn!("[LAN] 热词为空，已丢弃");
                return false;
            }
            if let Err(e) = crate::db::managers::hotword_repo::validate_text_len(text) {
                tracing::warn!("[LAN] 热词未通过长度校验，已丢弃: {e}");
                return false;
            }
            m.text = text.to_string();
            true
        },
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

    use crate::db::managers::hotword_repo::HotwordRepo;

    /// 畸形 payload 在应用阶段失败时不应上抛 Err（否则启动 panic），
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

    /// 行为变更：热词行的 `lang` 归一已删（列不存在）；旧 payload 里多出的 `lang`
    /// 被 serde 忽略，不报错、不丢词。`weight` 归一**仍在**。
    #[tokio::test]
    async fn mirror_ignores_legacy_lang_and_normalizes_weight() {
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

        // 源端存的是合法值，脏值是后面的 JSON 注入（DDL 的 CHECK 不允许直接写进源库）
        let mut records = export_all_records(src.path()).await.expect("导出失败");
        // 老设备发的 payload：热词行多一个 lang，weight 越界
        records.role_hotwords[0]["lang"] = serde_json::json!("zh-Hans");
        records.role_hotwords[0]["weight"] = serde_json::json!(999);

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        apply_records(&dst_db, &records).await.expect("导入应成功");

        let rows: Vec<(String, i32)> = HotwordRepo::list_by_role(&dst_db, 7)
            .await
            .expect("查询失败")
            .into_iter()
            .map(|m| (m.text, m.weight))
            .collect();
        assert_eq!(
            rows,
            vec![("张三".to_string(), 50)],
            "weight 仍归一到 50，lang 被忽略"
        );
    }

    /// 对端 payload 里的超长词条**丢弃并 warn**，不让整批同步失败：
    /// 同步是「另一台设备的完整镜像」，一行坏数据不该把 11 张表全部回滚。
    #[tokio::test]
    async fn mirror_drops_overlong_hotword_text() {
        // 照抄 `mirror_ignores_legacy_lang_and_normalizes_weight` 的手法：
        // 真导出 → 在 JSON 层注入 → 导入。**长度判据只在应用层**（DDL 的
        // `CHECK` 只管网 weight），所以脏值必须在 JSON 层注入，写不进源库。
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
            text: Set("合规词".to_string()),
            weight: Set(4),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入热词失败");

        let mut records = export_all_records(src.path()).await.expect("导出失败");
        assert_eq!(records.role_hotwords.len(), 1);
        // 注入第二条：16 个汉字（非 ASCII 分支，超 15 字符上限）
        records.role_hotwords.push(serde_json::json!({
            "id": 2, "role_id": 7, "text": "长".repeat(16), "weight": 4,
        }));

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        // 本批**不失败**：坏数据是丢行，不是丢批（对比 DELETE_ORDER 放错时的稳定失败）
        apply_records(&dst_db, &records)
            .await
            .expect("单条超长不该让整批同步失败");

        let texts: Vec<String> = HotwordRepo::list_by_role(&dst_db, 7)
            .await
            .expect("查询失败")
            .into_iter()
            .map(|m| m.text)
            .collect();
        assert_eq!(texts, vec!["合规词".to_string()], "超长行应被丢弃");
    }

    /// 对端 payload 里词首尾带空格的合法热词必须**以 trim 后的值入库**：
    /// `validate_text_len` 只判长度不管空白，而 `text` 是归并/唯一键——原样入库会把
    /// 同一个词拆成两个不同的 key，悄悄破坏同步本该保住的去重（本地写入路径先 trim，
    /// 这种值本地产不出来，只能在 JSON 层注入）。
    #[tokio::test]
    async fn mirror_trims_hotword_text_before_insert() {
        // 照抄 `mirror_drops_overlong_hotword_text` 的手法：真导出 → JSON 层注入 → 导入。
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
            weight: Set(4),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入热词失败");

        let mut records = export_all_records(src.path()).await.expect("导出失败");
        assert_eq!(records.role_hotwords.len(), 1);
        // 注入第二条：词首尾各一个空格，长度合法（3 个汉字），只是脏。
        // trim 后与源端的「张三」不同词，不会撞 `UNIQUE (role_id, text)`。
        records.role_hotwords.push(serde_json::json!({
            "id": 2, "role_id": 7, "text": " 合规词 ", "weight": 4,
        }));

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        let imported = apply_records(&dst_db, &records).await.expect("导入应成功");

        // 该行是「归一后导入」，不是「未通过校验丢弃」：计数必须含它。
        assert_eq!(imported, 3, "1 角色 + 2 热词都应导入，带空格的行不应被丢");

        let texts: Vec<String> = HotwordRepo::list_by_role(&dst_db, 7)
            .await
            .expect("查询失败")
            .into_iter()
            .map(|m| m.text)
            .collect();
        assert_eq!(
            texts,
            vec!["张三".to_string(), "合规词".to_string()],
            "带空格的词应以 trim 后的值入库；原样入库会把归并键拆成两个"
        );
    }

    /// 旧版本发出的 payload 没有热词相关的新字段；缺 `#[serde(default)]` 会解析失败，
    /// 而 `load_staged_db_records` 解析失败只 warn 后返回 None——同步静默失效。
    #[tokio::test]
    async fn old_payload_without_hotwords_does_not_error() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let data_dir = tmp.path();
        let db = crate::db::init_db(data_dir)
            .await
            .expect("初始化数据库失败");

        role::ActiveModel {
            id: Set(7),
            name: Set("甲".to_string()),
            role_type: Set(role::RoleType::Main),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("插入角色失败");

        // 拿一份真实导出删掉热词相关字段，模拟旧版本发出的 payload
        let records = export_all_records(data_dir).await.expect("导出失败");
        let mut payload = serde_json::to_value(&records).expect("序列化失败");
        let obj = payload.as_object_mut().expect("payload 应为 JSON 对象");
        assert!(
            obj.remove("roleHotwords").is_some(),
            "线上字段名应为 roleHotwords"
        );
        assert!(
            obj.remove("globalHotwords").is_some(),
            "线上字段名应为 globalHotwords"
        );
        assert!(
            obj.remove("hotwordCategories").is_some(),
            "线上字段名应为 hotwordCategories"
        );
        assert!(
            obj.remove("libraryHotwords").is_some(),
            "线上字段名应为 libraryHotwords"
        );

        let staging = staging_dir(data_dir);
        std::fs::create_dir_all(&staging).expect("创建暂存目录失败");
        std::fs::write(staging.join(DB_RECORDS_FILE), payload.to_string())
            .expect("写入暂存 payload 失败");

        let loaded = load_staged_db_records(data_dir).expect("旧 payload 应能解析");
        assert!(
            loaded.role_hotwords.is_empty() && loaded.global_hotwords.is_empty(),
            "缺失的字段应默认为空数组"
        );

        assert_eq!(
            apply_staged_db_records(&db, data_dir).await,
            Ok(1),
            "旧 payload 应正常导入"
        );
        assert!(
            !staging.join("db_records.json.bad").exists(),
            "不应走失败路径"
        );
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

    /// 行为变更：全局热词行的 `lang` 归一已删（列不存在）；旧 payload 多出的 `lang`
    /// 被忽略、不报错。越界 `weight` 仍在镜像入口归一（否则 CHECK 让整批镜像失败）。
    #[tokio::test]
    async fn mirror_ignores_legacy_global_lang_and_normalizes_weight() {
        let src = tempfile::tempdir().expect("创建临时目录失败");
        let src_db = crate::db::init_db(src.path())
            .await
            .expect("初始化源数据库失败");

        global_hotword::ActiveModel {
            text: Set("全局词".to_string()),
            weight: Set(9),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入全局热词失败");

        // 真导出后在 JSON 层注入脏值（DDL 的 CHECK 不允许直接写进源库）
        let mut records = export_all_records(src.path()).await.expect("导出失败");
        // 老设备发的 payload：热词行多一个 lang，weight 越界
        records.global_hotwords[0]["lang"] = serde_json::json!("zh-Hans");
        records.global_hotwords[0]["weight"] = serde_json::json!(999);

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        stage_db_records(dst.path(), &records).expect("暂存失败");

        assert_eq!(
            apply_staged_db_records(&dst_db, dst.path()).await,
            Ok(1),
            "归一后应正常导入，而不是撞 CHECK 整批失败"
        );

        let hotwords = global_hotword::Entity::find()
            .all(&dst_db)
            .await
            .expect("查询全局热词失败");
        assert_eq!(hotwords.len(), 1);
        assert_eq!(hotwords[0].weight, 50, "越界 weight 应 clamp");
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

    /// 行为变更：词库行的 `lang` 归一已删（列不存在）；旧 payload 多出的 `lang`
    /// 被忽略、不报错。越界 `weight` 仍在镜像入口归一。
    #[tokio::test]
    async fn mirror_ignores_legacy_library_lang_and_normalizes_weight() {
        let src = tempfile::tempdir().expect("创建临时目录失败");
        let src_db = crate::db::init_db(src.path())
            .await
            .expect("初始化源数据库失败");
        library_hotword::ActiveModel {
            category_id: Set(None),
            text: Set("词".to_string()),
            weight: Set(9),
            ..Default::default()
        }
        .insert(&src_db)
        .await
        .expect("插入失败");

        let mut records = export_all_records(src.path()).await.expect("导出失败");
        // 老设备发的 payload：词条行多一个 lang，weight 越界
        records.library_hotwords[0]["lang"] = serde_json::json!("zh-Hans");
        records.library_hotwords[0]["weight"] = serde_json::json!(999);

        let dst = tempfile::tempdir().expect("创建临时目录失败");
        let dst_db = crate::db::init_db(dst.path())
            .await
            .expect("初始化目标数据库失败");
        stage_db_records(dst.path(), &records).expect("暂存失败");

        assert_eq!(
            apply_staged_db_records(&dst_db, dst.path()).await,
            Ok(1),
            "归一后应正常导入"
        );

        let words = library_hotword::Entity::find()
            .all(&dst_db)
            .await
            .expect("查询失败");
        assert_eq!(words[0].weight, 50, "越界 weight 应 clamp");
    }
}
