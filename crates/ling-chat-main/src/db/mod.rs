pub mod compat;
pub mod entities;
pub mod managers;
pub mod role_sync;

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, EntityTrait, Set};
use sea_orm_migration::seaql_migrations;
use sea_orm_migration::{MigratorTrait, SchemaManager};

use crate::migration::Migrator;

pub async fn init_db(data_dir: &Path) -> Result<DatabaseConnection> {
    std::fs::create_dir_all(data_dir)?;

    let db_path = data_dir.join("game_database.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let db = Database::connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    // Detect and migrate old Python-backend databases before running standard migrations
    if let Err(e) = compat::migrate_from_python(&db, data_dir).await {
        // Log the full error chain since Tauri's panic only shows the outermost message
        for cause in e.chain() {
            tracing::error!("compat migration error: {cause}");
        }
        return Err(e).context("Failed to migrate database from old Python schema");
    }

    apply_pending_migrations(&db)
        .await
        .map_err(|e: sea_orm::DbErr| {
            // Tauri only prints the outermost context, so log the full chain here.
            tracing::error!("migration error: {e}");
            e
        })
        .context("Failed to run database migrations")?;

    tracing::info!("Database initialized at {:?}", db_path);
    Ok(db)
}

/// 应用待执行迁移，忽略数据库中「本构建没有对应迁移文件」的记录。
///
/// 不能直接用 `Migrator::up`：`sea_orm_migration` 的 `get_migration_with_status()`
/// 一旦发现「已应用、但当前构建里找不到迁移文件」的记录就直接报错，而不是容忍它。
/// 于是跨分支/降级场景（例如热词三表合一后旧库仍记着六条已删除的迁移）会在启动
/// 阶段失败；Android 上表现为「打开即闪退」，且原因被最外层 context 盖住。
///
/// 这里照 sea-orm 自己的流程重跑一遍（install → 逐条 up → 记账），只把「库里有、
/// 本构建没有」的记录当噪音忽略：不删除记录、不碰业务表，其它分支的账本保持完整，
/// 切回去也不会重复执行迁移。
async fn apply_pending_migrations(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    // 建账本表（幂等）。全新库上它还不存在，后续查询会失败。
    Migrator::install(db).await?;

    let applied: HashSet<String> = seaql_migrations::Entity::find()
        .all(db)
        .await?
        .into_iter()
        .map(|row| row.version)
        .collect();

    let migrations = Migrator::migrations();
    let known: HashSet<&str> = migrations.iter().map(|m| m.name()).collect();
    let foreign: Vec<&str> = applied
        .iter()
        .map(String::as_str)
        .filter(|version| !known.contains(version))
        .collect();
    if !foreign.is_empty() {
        tracing::warn!(
            "数据库包含 {} 条本构建不认识的迁移记录（通常来自其它分支构建），已忽略: {foreign:?}",
            foreign.len()
        );
    }

    let manager = SchemaManager::new(db);
    for migration in &migrations {
        let name = migration.name();
        if applied.contains(name) {
            continue;
        }
        tracing::info!("Applying migration '{name}'");
        migration.up(&manager).await?;
        seaql_migrations::ActiveModel {
            version: Set(name.to_owned()),
            applied_at: Set(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_secs() as i64)
                .unwrap_or_default()),
        }
        .insert(db)
        .await?;
    }

    Ok(())
}
