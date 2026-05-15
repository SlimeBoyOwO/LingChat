pub mod entities;
pub mod managers;

use std::path::Path;

use anyhow::{Context, Result};
use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;

use crate::migration::Migrator;

pub async fn init_db(data_dir: &Path) -> Result<DatabaseConnection> {
    std::fs::create_dir_all(data_dir)?;

    let db_path = data_dir.join("game_database.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let db = Database::connect(&db_url)
        .await
        .context("Failed to connect to database")?;

    Migrator::up(&db, None)
        .await
        .context("Failed to run database migrations")?;

    log::info!("Database initialized at {:?}", db_path);
    Ok(db)
}
