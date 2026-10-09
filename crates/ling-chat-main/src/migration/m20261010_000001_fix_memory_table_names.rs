use sea_orm::{ConnectionTrait, DbErr};
use sea_orm_migration::prelude::*;

/// 修正 `m20261007` / `m20261009` 建错的表名（单数 vs 复数）。
///
/// 两个新迁移当时用 `DeriveIden` 的默认命名，把表建成了单数
/// `memory_embedding` 与 `memory_archive_profile`；而代码（实体、记忆存储层、
/// LAN 同步、目录表读写）从头到尾只用复数 `memory_embeddings` 与
/// `memory_archive_profiles`。后果：迁移记录虽然存在，但这些表的读写会报
/// `no such table`（真实的「迁移已记录但表不存在」缺表场景）。
///
/// 运行时的 `ensure_*` 兜底只负责补建复数空表，掩盖了症状、错误名的单数空表
/// 一直残留。本迁移做两件事：
/// 1. 删除两张错误命名的单数表（历史写入始终走复数表，单数表不会有数据）；
/// 2. 按代码实际使用的复数表名补建（`IF NOT EXISTS` 幂等，已有则跳过）。
#[derive(DeriveMigrationName)]
pub struct Migration;

const MEMORY_EMBEDDINGS_DDL: &str = r#"CREATE TABLE IF NOT EXISTS memory_embeddings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  role_id INTEGER NOT NULL,
  memory_id VARCHAR(64) NOT NULL,
  chunk_index INTEGER NOT NULL,
  content TEXT NOT NULL,
  tags TEXT NULL,
  embedding BLOB NOT NULL,
  dim INTEGER NOT NULL,
  model VARCHAR(128) NOT NULL,
  created_at INTEGER NOT NULL
)"#;

const MEMORY_ARCHIVE_PROFILES_DDL: &str = r#"CREATE TABLE IF NOT EXISTS memory_archive_profiles (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  script_key TEXT,
  script_role_key TEXT,
  role_id INTEGER NOT NULL UNIQUE,
  role_name TEXT NOT NULL,
  table_name TEXT NOT NULL UNIQUE,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
)"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1) 删除错误命名的单数表（IF EXISTS：全新库上这两张表根本不存在）。
        db.execute_unprepared("DROP TABLE IF EXISTS memory_embedding")
            .await?;
        db.execute_unprepared("DROP TABLE IF EXISTS memory_archive_profile")
            .await?;

        // 2) 按代码实际使用的复数表名补建（幂等）。
        db.execute_unprepared(MEMORY_EMBEDDINGS_DDL).await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_memory_embeddings_role \
             ON memory_embeddings (role_id)",
        )
        .await?;
        db.execute_unprepared(MEMORY_ARCHIVE_PROFILES_DDL).await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
