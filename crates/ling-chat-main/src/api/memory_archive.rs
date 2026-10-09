//! 旧版存档记忆的一键整合导入。
//!
//! 旧版（升级前）的四段式记忆存在 `memory_bank` 表：按 `save_id` × `role_id`
//! 一行一档，`info` 列是 `GameMemoryBank` JSON（`data.short_term / long_term /
//! user_info / promises` 四段）。新版记忆系统 `memory_embeddings` 只按 `role_id`
//! 隔离、不再按存档分档，旧表在升级后变得不可见。
//!
//! 用户要求提供「一键整合」：把旧表里每个「剧本 × 角色」的记忆整合出来，为每个
//! 组合单独建一张 SQL 表归档。实现方式：
//! - 读取旧 `memory_bank`，按「同一存档的最新记录」归约后，跨存档合并四段内容
//!   （跳过占位符与空文本）；
//! - 为每个角色（每个角色唯一属于一个剧本）创建数据表 `memory_archive_{role_id}`；
//! - 元信息记入目录表 `memory_archive_profiles`（见 `migration/m20261009`），
//!   供设置页「记忆系统」面板浏览 / 查看 / 删除。

use std::collections::HashMap;

use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, Statement,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::AppState;
use crate::ai_service::memory::store::{ensure_archive_table_schema, ensure_role_archive};
use crate::db::entities::memory_archive_profile;

/// 各段的默认占位文本（旧版空内容时的占位符，跳过不导入）。
const PLACEHOLDERS: [&str; 4] = [
    "暂无近期对话摘要。",
    "暂无长期关键经历。",
    "暂无用户特征记录。",
    "暂无未完成的约定。",
];

/// 旧版 `memory_bank.info` 的 JSON 结构（`GameMemoryBank` 兼容解析）。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyBankInfo {
    data: LegacyBankData,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct LegacyBankData {
    short_term: String,
    long_term: String,
    user_info: String,
    promises: String,
}

impl LegacyBankInfo {
    /// 收集四个分段的非空、非占位文本，key 为段名。
    fn meaningful_sections(&self) -> Vec<(String, String)> {
        let mut out = Vec::with_capacity(4);
        for (name, text) in [
            ("short_term", &self.data.short_term),
            ("long_term", &self.data.long_term),
            ("user_info", &self.data.user_info),
            ("promises", &self.data.promises),
        ] {
            let trimmed = text.trim();
            if !trimmed.is_empty() && !PLACEHOLDERS.iter().any(|p| trimmed == *p) {
                out.push((name.to_string(), trimmed.to_string()));
            }
        }
        out
    }
}

/// 导入结果汇总。
#[derive(Clone, Serialize)]
pub struct LegacyImportResult {
    /// 旧表中读到的存档记忆行数。
    pub legacy_rows: usize,
    /// 新建档案的角色数（每个角色 = 一个剧本 × 角色 组合）。
    pub imported_roles: usize,
    /// 目录表里已存在、本次跳过未重复导入的角色数。
    pub skipped_existing: usize,
    /// 实际写入的整合记忆条目数（按分段计）。
    pub memories_written: usize,
    /// 内容为空、未建档案的角色数。
    pub skipped_empty: usize,
    /// 无法归属到角色的行数（`role_id` 为空）。
    pub skipped_unassigned: usize,
}

/// 档案目录摘要（设置页列表展示）。
#[derive(Clone, Serialize)]
pub struct MemoryArchiveSummary {
    pub id: i32,
    pub role_id: i32,
    pub role_name: String,
    pub script_key: Option<String>,
    pub script_role_key: Option<String>,
    /// 数据表名（`memory_archive_{role_id}`）。
    pub table_name: String,
    /// 记忆条目数。
    pub entry_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 档案内的一条记忆。
#[derive(Clone, Serialize)]
pub struct MemoryArchiveEntry {
    pub id: i32,
    pub section: String,
    pub content: String,
    pub created_at: i64,
}

/// 旧表原始行（`memory_bank` 实体已删，走原生 SQL）。
#[derive(Debug, FromQueryResult)]
struct BankRow {
    id: i32,
    info: String,
    save_id: i32,
    role_id: Option<i32>,
}

/// 动态数据表内 COUNT 的结果。
#[derive(Debug, FromQueryResult)]
struct CountRow {
    #[sea_orm(column_name = "c")]
    c: i64,
}

/// 目录表 `memory_archive_profiles` 的兜底建表（防御旧库升级时迁移未落表、或
/// 数据库文件被替换等场景）。与 `migration/m20261009` 的 DDL 保持一致；
/// `CREATE TABLE IF NOT EXISTS` / 索引幂等，命令入口各调一次开销可忽略。
async fn ensure_profile_table(db: &DatabaseConnection) -> Result<(), String> {
    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        r#"CREATE TABLE IF NOT EXISTS memory_archive_profiles (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             script_key TEXT,
             script_role_key TEXT,
             role_id INTEGER NOT NULL UNIQUE,
             role_name TEXT NOT NULL,
             table_name TEXT NOT NULL UNIQUE,
             created_at INTEGER NOT NULL,
             updated_at INTEGER NOT NULL
           )"#,
    ))
    .await
    .map_err(|e| format!("确保档案目录表存在失败: {e}"))?;
    Ok(())
}

/// `memory_archive_{id}` 数据表结构见 `store::MEMORY_ARCHIVE_DDL`（与长期记忆共用）。

/// 一键导入旧版存档记忆：把 `memory_bank` 的四段式记忆整合到
/// 每「剧本 × 角色」一张的独立归档表中。
///
/// 幂等：目录表里已有的角色档案不会被重复导入（返回 `skipped_existing`）。
#[tauri::command]
pub async fn memory_import_legacy(app: tauri::AppHandle) -> Result<LegacyImportResult, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    // 1. 确认旧表存在（全新库可能没有用完的 migration 从未建表）。
    let exists = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table' AND name='memory_bank'",
        ))
        .await
        .map_err(|e| format!("检查旧版记忆表失败: {e}"))?;
    if exists.is_none() {
        return Ok(LegacyImportResult {
            legacy_rows: 0,
            imported_roles: 0,
            skipped_existing: 0,
            memories_written: 0,
            skipped_empty: 0,
            skipped_unassigned: 0,
        });
    }

    // 1.5 防御旧库升级/库被替换导致目录表缺失：幂等补建。
    ensure_profile_table(db).await?;

    // 2. 读取全部旧记忆行。
    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT id, info, save_id, role_id FROM memory_bank ORDER BY id",
        ))
        .await
        .map_err(|e| format!("读取旧版记忆失败: {e}"))?;
    let mut banks: Vec<BankRow> = Vec::with_capacity(rows.len());
    for row in &rows {
        if let Ok(r) = BankRow::from_query_result(row, "") {
            banks.push(r);
        } else {
            tracing::warn!("解析 memory_bank 行失败（跳过）");
        }
    }

    // 3. 按「存档 × 角色」归约到最新一条（对齐旧版载入逻辑：同一存档每角色取 id 最大）。
    let mut latest: HashMap<(i32, i32), BankRow> = HashMap::new(); // (save_id, role_id) -> row
    let mut skipped_unassigned = 0usize;
    for row in banks {
        let Some(role_id) = row.role_id else {
            skipped_unassigned += 1; // role_id 为空的行无法归属角色
            continue;
        };
        match latest.get(&(row.save_id, role_id)) {
            Some(prev) if prev.id >= row.id => {},
            _ => {
                latest.insert((row.save_id, role_id), row);
            },
        }
    }

    let legacy_rows = latest.len();

    // 4. 按角色聚合，跨存档合并四段内容。
    let mut per_role: HashMap<i32, Vec<(String, String)>> = HashMap::new(); // role_id -> sections
    for ((_save_id, role_id), row) in &latest {
        let parsed: LegacyBankInfo = match serde_json::from_str(&row.info) {
            Ok(v) => v,
            Err(_) => LegacyBankInfo::default(),
        };
        per_role
            .entry(*role_id)
            .or_default()
            .extend(parsed.meaningful_sections());
    }

    // 5. 角色归并（无元数据预取：建档案时由 ensure_role_archive 查 role 表）。
    let now = chrono::Utc::now().timestamp_millis();

    let mut imported_roles = 0usize;
    let mut skipped_existing = 0usize;
    let mut memories_written = 0usize;
    let mut skipped_empty = 0usize;

    for (role_id, mut sections) in per_role {
        // 去重同一分段内的重复文本（跨存档合并出现同一内容时保留一份，保序）。
        let mut seen = Vec::new();
        sections.retain(|(section, text)| {
            let key = format!("{section}\u{0}{text}");
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        });
        if sections.is_empty() {
            skipped_empty += 1;
            continue;
        }

        // 幂等：已有档案则跳过（含长期记忆已建档案的角色）。
        let existing = memory_archive_profile::Entity::find()
            .filter(memory_archive_profile::Column::RoleId.eq(role_id))
            .one(db)
            .await
            .map_err(|e| format!("检查档案目录失败: {e}"))?;
        if existing.is_some() {
            skipped_existing += 1;
            continue;
        }

        // 共用归档表：与长期记忆同一张「剧本 × 角色」表（建表 + 目录行）。
        let table = ensure_role_archive(db, role_id).await?;

        for (section, content) in &sections {
            db.execute(Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                format!(
                    "INSERT INTO \"{table}\" (kind, section, content, created_at) \
                     VALUES ('legacy', ?, ?, ?)"
                ),
                vec![
                    sea_orm::Value::String(Some(Box::new(section.clone()))),
                    sea_orm::Value::String(Some(Box::new(content.clone()))),
                    sea_orm::Value::BigInt(Some(now)),
                ],
            ))
            .await
            .map_err(|e| format!("写入档案表 {table} 失败: {e}"))?;
            memories_written += 1;
        }
        imported_roles += 1;
    }

    Ok(LegacyImportResult {
        legacy_rows,
        imported_roles,
        skipped_existing,
        memories_written,
        skipped_empty,
        skipped_unassigned,
    })
}

/// 列出所有已导入的「剧本 × 角色」记忆档案（按创建时间倒序）。
#[tauri::command]
pub async fn memory_archive_list(
    app: tauri::AppHandle,
) -> Result<Vec<MemoryArchiveSummary>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    ensure_profile_table(db).await?;

    let profiles = memory_archive_profile::Entity::find()
        .order_by_desc(memory_archive_profile::Column::Id)
        .all(db)
        .await
        .map_err(|e| format!("读取档案目录失败: {e}"))?;

    let mut out = Vec::with_capacity(profiles.len());
    for p in profiles {
        ensure_archive_table_schema(db, &p.table_name).await?;
        let count = db
            .query_one(Statement::from_string(
                DatabaseBackend::Sqlite,
                format!(
                    "SELECT COUNT(*) AS c FROM \"{}\" WHERE kind = 'legacy'",
                    p.table_name
                ),
            ))
            .await
            .and_then(|opt| opt.map(|r| CountRow::from_query_result(&r, "")).transpose())
            .map(|opt| opt.map(|c| c.c).unwrap_or(0))
            .unwrap_or(0);
        out.push(MemoryArchiveSummary {
            id: p.id,
            role_id: p.role_id,
            role_name: p.role_name,
            script_key: p.script_key,
            script_role_key: p.script_role_key,
            table_name: p.table_name,
            entry_count: count,
            created_at: p.created_at,
            updated_at: p.updated_at,
        });
    }
    Ok(out)
}

/// 查看某个「剧本 × 角色」档案里的全部记忆条目（按分段名排序）。
#[tauri::command]
pub async fn memory_archive_get(
    app: tauri::AppHandle,
    profile_id: i32,
) -> Result<Vec<MemoryArchiveEntry>, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    ensure_profile_table(db).await?;

    let profile = memory_archive_profile::Entity::find_by_id(profile_id)
        .one(db)
        .await
        .map_err(|e| format!("读取档案目录失败: {e}"))?;
    let Some(profile) = profile else {
        return Err(format!("档案不存在（id={profile_id}）"));
    };

    ensure_archive_table_schema(db, &profile.table_name).await?;

    #[derive(Debug, FromQueryResult)]
    struct EntryRow {
        id: i32,
        section: String,
        content: String,
        created_at: i64,
    }

    let rows = db
        .query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT id, section, content, created_at FROM \"{}\" \
                 WHERE kind = 'legacy' ORDER BY section, id",
                profile.table_name
            ),
        ))
        .await
        .map_err(|e| format!("读取档案内容失败: {e}"))?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in &rows {
        if let Ok(r) = EntryRow::from_query_result(row, "") {
            entries.push(MemoryArchiveEntry {
                id: r.id,
                section: r.section,
                content: r.content,
                created_at: r.created_at,
            });
        }
    }
    Ok(entries)
}

/// 删除某个「剧本 × 角色」档案里的**旧版整合条目**（`kind='legacy'`）。
///
/// 共用归档表还承载长期记忆（`kind='memory'`）：只有当表里不再有任何长期记忆时
/// 才连同数据表和目录行一起清理；仍存在长期记忆则保留表与目录行（档案面板仅移除
/// 整合条目）。返回是否真的删除了条目。
#[tauri::command]
pub async fn memory_archive_delete(app: tauri::AppHandle, profile_id: i32) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let db = &state.db;

    ensure_profile_table(db).await?;

    let profile = memory_archive_profile::Entity::find_by_id(profile_id)
        .one(db)
        .await
        .map_err(|e| format!("读取档案目录失败: {e}"))?;
    let Some(profile) = profile else {
        return Ok(false);
    };

    ensure_archive_table_schema(db, &profile.table_name).await?;

    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        format!(
            "DELETE FROM \"{}\" WHERE kind = 'legacy'",
            profile.table_name
        ),
    ))
    .await
    .map_err(|e| format!("删除档案条目失败: {e}"))?;

    // 若表里已无长期记忆（共用表只装整合条目了），整表 + 目录行一并清理；
    // 否则保留（该角色的长期记忆还在这里）。
    let memory_left = db
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            format!(
                "SELECT 1 AS x FROM \"{}\" WHERE kind = 'memory' LIMIT 1",
                profile.table_name
            ),
        ))
        .await
        .map_err(|e| format!("检查长期记忆残留失败: {e}"))?
        .is_some();

    if memory_left {
        return Ok(true);
    }

    db.execute(Statement::from_string(
        DatabaseBackend::Sqlite,
        format!("DROP TABLE IF EXISTS \"{}\"", profile.table_name),
    ))
    .await
    .map_err(|e| format!("删除档案表失败: {e}"))?;

    let deleted = memory_archive_profile::Entity::delete_by_id(profile.id)
        .exec(db)
        .await
        .map_err(|e| format!("删除档案目录失败: {e}"))?;
    Ok(deleted.rows_affected > 0)
}
