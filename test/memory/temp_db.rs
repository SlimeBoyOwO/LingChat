//! Isolated SQLite boundary for memory validation.
//!
//! Each instance owns a unique temporary directory and seeds the minimum valid
//! role/save foreign-key graph before repository round-trip operations.

use anyhow::{Context, Result, anyhow};
use chrono::Local;
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, Set};
use std::path::{Path, PathBuf};

use crate::ai_service::types::GameMemoryBank;
use crate::db::entities::role::RoleType;
use crate::db::entities::{memory_bank, role, save};
use crate::db::managers::memory_repo::MemoryRepo;

pub const PRODUCTION_DATABASE_PATH: &str = "data/game_database.db";

pub fn assert_is_test_database(path: &Path) {
    let production = Path::new(PRODUCTION_DATABASE_PATH);
    assert_ne!(
        path, production,
        "validation must never use the production DB"
    );
}

pub struct TemporaryDatabase {
    pub directory: tempfile::TempDir,
    pub connection: DatabaseConnection,
}

impl TemporaryDatabase {
    pub async fn open() -> Result<Self> {
        let directory = tempfile::tempdir().context("create temporary memory DB directory")?;
        assert_is_test_database(directory.path());
        let connection = crate::db::init_db(directory.path())
            .await
            .context("run migrations for temporary memory DB")?;
        Ok(Self {
            directory,
            connection,
        })
    }

    pub fn path(&self) -> PathBuf {
        self.directory.path().join("game_database.db")
    }

    /// Open a second pool for tests that need to hold a real SQLite writer lock
    /// while the production repository performs a save.
    pub async fn secondary_connection(&self) -> Result<DatabaseConnection> {
        let path = self.path();
        Database::connect(format!("sqlite:{}?mode=rw", path.display()))
            .await
            .context("open secondary test database connection")
    }

    pub async fn seed_save_role(&self, role_id: i32, title: &str) -> Result<(i32, i32)> {
        let role = role::ActiveModel {
            id: Set(role_id),
            name: Set(title.to_string()),
            role_type: Set(RoleType::Npc),
            ..Default::default()
        }
        .insert(&self.connection)
        .await?;
        let now = Local::now().naive_local();
        let save = save::ActiveModel {
            title: Set(format!("test-{title}")),
            status: Set("{}".into()),
            create_date: Set(now),
            update_date: Set(now),
            main_role_id: Set(Some(role.id)),
            ..Default::default()
        }
        .insert(&self.connection)
        .await?;
        Ok((save.id, role.id))
    }

    /// Insert a main role usable only for failure-path validation. It points
    /// at a missing resource/settings folder, so production lazy loading fails
    /// after a preview transition has been admitted.
    pub async fn seed_unloadable_role(&self, role_id: i32, title: &str) -> Result<i32> {
        Ok(role::ActiveModel {
            id: Set(role_id),
            name: Set(title.to_string()),
            role_type: Set(RoleType::Main),
            resource_folder: Set(Some("missing-preview-settings".into())),
            ..Default::default()
        }
        .insert(&self.connection)
        .await?
        .id)
    }

    /// Seed a minimal MAIN role that production lazy loading can register.
    /// The settings file is intentionally local to this temporary data root.
    pub async fn seed_loadable_main_role(&self, role_id: i32, folder: &str) -> Result<i32> {
        let path = self
            .directory
            .path()
            .join("game_data/characters")
            .join(folder);
        std::fs::create_dir_all(&path)?;
        std::fs::write(
            path.join("settings.yml"),
            format!("ai_name: {folder}\nuser_name: tester\nsystem_prompt: preview persona\n"),
        )?;
        Ok(role::ActiveModel {
            id: Set(role_id),
            name: Set(folder.to_string()),
            role_type: Set(RoleType::Main),
            resource_folder: Set(Some(folder.to_string())),
            ..Default::default()
        }
        .insert(&self.connection)
        .await?
        .id)
    }

    pub async fn round_trip(
        &self,
        save_id: i32,
        role_id: i32,
        bank: &GameMemoryBank,
    ) -> Result<GameMemoryBank> {
        MemoryRepo::upsert_for_save_role(&self.connection, save_id, role_id, bank).await?;
        let row = MemoryRepo::get_latest_memory(&self.connection, save_id, role_id)
            .await?
            .ok_or_else(|| anyhow!("memory row missing after upsert"))?;
        serde_json::from_str(&row.info).with_context(|| {
            format!(
                "decode memory bank save_id={save_id} role_id={role_id} row_id={}",
                row.id
            )
        })
    }

    pub async fn malformed_json_error(&self, save_id: i32, role_id: i32) -> Result<String> {
        memory_bank::ActiveModel {
            info: Set("{not-json".into()),
            save_id: Set(save_id),
            role_id: Set(Some(role_id)),
            ..Default::default()
        }
        .insert(&self.connection)
        .await?;
        let row = MemoryRepo::get_latest_memory(&self.connection, save_id, role_id)
            .await?
            .ok_or_else(|| anyhow!("malformed memory row missing"))?;
        serde_json::from_str::<GameMemoryBank>(&row.info)
            .map(|_| "unexpectedly decoded malformed JSON".into())
            .map_err(|e| {
                anyhow!(
                    "malformed memory JSON save_id={save_id} role_id={role_id} row_id={}: {e}",
                    row.id
                )
            })
    }
}
