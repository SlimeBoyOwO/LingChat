pub mod compat;
pub mod entities;
pub mod managers;

use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::migration::Migrator;

pub async fn init_db(data_dir: &Path) -> Result<DatabaseConnection> {
    std::fs::create_dir_all(data_dir)?;

    let db_path = data_dir.join("game_database.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let db = Database::connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    // 提高外部存储（Android /storage/emulated/0/.../files，emulated/FUSE）上的写入稳定性：
    // - WAL：写入走 append-only 日志，避免每次事务重建 rollback journal，掉电/被杀更不容易坏库
    // - synchronous=NORMAL：配合 WAL，性能与安全折中（只在 checkpoint 时 fsync）
    // - busy_timeout：外部存储上偶发锁竞争，等而不是立刻报 SQLITE_BUSY
    for pragma in [
        "PRAGMA journal_mode = WAL;",
        "PRAGMA synchronous = NORMAL;",
        "PRAGMA busy_timeout = 5000;",
    ] {
        db.execute_unprepared(pragma)
            .await
            .with_context(|| format!("Failed to execute `{pragma}`"))?;
    }

    // Detect and migrate old Python-backend databases before running standard migrations
    if let Err(e) = compat::migrate_from_python(&db, data_dir).await {
        // Log the full error chain since Tauri's panic only shows the outermost message
        for cause in e.chain() {
            tracing::error!("compat migration error: {cause}");
        }
        return Err(e).context("Failed to migrate database from old Python schema");
    }

    Migrator::up(&db, None)
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
