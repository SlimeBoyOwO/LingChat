use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait};

/// Execute a raw SQL statement with per-step context for debugging.
async fn exec(db: &impl ConnectionTrait, description: &str, stmt: &str) -> Result<()> {
    db.execute(Statement::from_string(DatabaseBackend::Sqlite, stmt))
        .await
        .with_context(|| format!("Migration step failed: {description}"))?;
    Ok(())
}

/// Check if the database was created by the old Python backend.
/// The old backend has a `user_info` table that the new Tauri schema removed.
async fn is_old_schema(db: &DatabaseConnection) -> bool {
    db.query_one(Statement::from_string(
        DatabaseBackend::Sqlite,
        "SELECT name FROM sqlite_master WHERE type='table' AND name='user_info'",
    ))
    .await
    .map(|r| r.is_some())
    .unwrap_or(false)
}

/// Migrate an old Python-backend database to the current Tauri schema.
///
/// Changes:
/// - `save`: drops the `user_id` column (FK → user_info)
/// - `adventure_unlock`: drops `user_id`, changes unique constraint from
///   `(user_id, adventure_folder)` to `adventure_folder` alone, adds index
/// - `adventure_progress`: table dropped entirely (removed in Tauri schema)
/// - `user_info`: table dropped entirely
///
/// Safe to call on already-migrated or fresh databases (no-op).
pub async fn migrate_from_python(db: &DatabaseConnection, data_dir: &Path) -> Result<()> {
    if !is_old_schema(db).await {
        tracing::debug!("Database schema is current, skipping compat migration");
        return Ok(());
    }

    tracing::info!("Detected old Python database, starting migration...");

    // Backup before touching anything — only if a backup doesn't already exist
    let db_path = data_dir.join("game_database.db");
    let backup_path = data_dir.join("game_database.backup.db");
    if !backup_path.exists() {
        std::fs::copy(&db_path, &backup_path)
            .context("Failed to create database backup before migration")?;
        tracing::info!("Backup saved to {:?}", backup_path);
    }

    // Disable FK enforcement at the connection level before starting the transaction.
    // SeaORM's transaction does not reliably propagate PRAGMAs, so this must be
    // set on the raw connection first.
    exec(db, "disable FK checks", "PRAGMA foreign_keys = OFF").await?;

    let txn = db.begin().await.context("Failed to begin transaction")?;

    // 1. Normalize data from old Python backend to Tauri schema conventions
    exec(&txn, "lowercase line attribute", "UPDATE line SET attribute = LOWER(attribute)").await?;
    exec(&txn, "lowercase role role_type", "UPDATE role SET role_type = LOWER(role_type)").await?;
    // Old backend used "current_character_id", new one uses "current_role_id"
    exec(
        &txn,
        "rename current_character_id in save status",
        "UPDATE save SET status = REPLACE(status, '\"current_character_id\"', '\"current_role_id\"')",
    )
    .await?;

    // 2. Drop user_info and adventure_progress first — they hold FKs pointing to
    //    save/user_info that would interfere with table rewrites below.
    exec(&txn, "drop adventure_progress (legacy)", "DROP TABLE IF EXISTS adventure_progress").await?;
    exec(&txn, "drop user_info table", "DROP TABLE user_info").await?;

    // 3. Recreate save without user_id column.
    //    ALTER TABLE DROP COLUMN is not used because it internally rewrites the
    //    table and fails when there are circular FK references (user_info ↔ save).
    exec(
        &txn,
        "create new save table",
        "CREATE TABLE save_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT '{}',
            create_date DATETIME NOT NULL,
            update_date DATETIME NOT NULL,
            running_script_id INTEGER,
            last_message_id INTEGER,
            main_role_id INTEGER,
            FOREIGN KEY(main_role_id) REFERENCES role(id)
        )",
    )
    .await?;

    exec(
        &txn,
        "copy save data",
        "INSERT INTO save_new
            (id, title, status, create_date, update_date, running_script_id, last_message_id, main_role_id)
         SELECT id, title, status, create_date, update_date, running_script_id, last_message_id, main_role_id
         FROM save",
    )
    .await?;

    exec(&txn, "drop old save", "DROP TABLE save").await?;
    exec(&txn, "rename save_new", "ALTER TABLE save_new RENAME TO save").await?;

    // 3. Recreate adventure_unlock without user_id and with corrected unique constraint
    exec(
        &txn,
        "create new adventure_unlock table",
        "CREATE TABLE adventure_unlock_new (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            adventure_folder TEXT NOT NULL UNIQUE,
            character_folder TEXT NOT NULL,
            unlocked_at TEXT,
            completed_at TEXT
        )",
    )
    .await?;

    exec(
        &txn,
        "copy adventure_unlock data",
        "INSERT INTO adventure_unlock_new
            (id, adventure_folder, character_folder, unlocked_at, completed_at)
         SELECT id, adventure_folder, character_folder, unlocked_at, completed_at
         FROM adventure_unlock",
    )
    .await?;

    exec(&txn, "drop old adventure_unlock", "DROP TABLE adventure_unlock").await?;
    exec(
        &txn,
        "rename adventure_unlock_new",
        "ALTER TABLE adventure_unlock_new RENAME TO adventure_unlock",
    )
    .await?;

    exec(
        &txn,
        "create ix_adventure_character index",
        "CREATE INDEX IF NOT EXISTS ix_adventure_character ON adventure_unlock(character_folder)",
    )
    .await?;

    txn.commit()
        .await
        .context("Failed to commit migration transaction")?;

    // Re-enable FK enforcement after the transaction is fully committed
    exec(db, "re-enable FK checks", "PRAGMA foreign_keys = ON").await?;

    tracing::info!("Database migration completed successfully");
    Ok(())
}
