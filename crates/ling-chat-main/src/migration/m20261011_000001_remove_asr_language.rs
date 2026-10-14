use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 删掉两张识别语言表（「识别语言」功能整体废弃，见计划 2026-10-11）。
///
/// ⚠️ 必须 `IF EXISTS`：迁移测试各自建**最小的**内存库、只跑自己那条迁移。本文件的
/// `is_idempotent_on_missing_tables`（什么都不建）与 `keeps_hotword_tables_intact`
/// （只建四张热词表）都不会建 config 表——裸 `DROP TABLE` 会让这两条红。
/// 实测：去掉 `IF EXISTS` → `1 passed; 2 failed`（`no such table: role_asr_config`）。
///
/// 整段自带 `BEGIN; ... COMMIT;`：与 `m20261010_000001` 同一取舍——sea-orm 1.x 只给
/// Postgres 的迁移开事务（`migrator.rs` 的 `exec_with_connection`），SQLite 走裸执行，
/// 失败会留下半成品。
const RAW_SQL: &str = "\
BEGIN;
DROP TABLE IF EXISTS role_asr_config;
DROP TABLE IF EXISTS global_asr_config;
COMMIT;
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(RAW_SQL).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 数据已丢弃，无法还原：与 m20261010_000001 一致只做 no-op 提示。
        Err(DbErr::Migration(
            "m20261011_000001 不可回滚：识别语言数据已删除".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{Database, DatabaseBackend, DatabaseConnection, Statement};

    async fn table_names(db: &DatabaseConnection) -> Vec<String> {
        db.query_all(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT name FROM sqlite_master WHERE type='table'",
        ))
        .await
        .expect("查询表失败")
        .into_iter()
        .map(|r| r.try_get::<String>("", "name").expect("读表名失败"))
        .collect()
    }

    #[tokio::test]
    async fn drops_both_config_tables() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("连接失败");
        db.execute_unprepared(
            "CREATE TABLE role_asr_config (
                 role_id INTEGER NOT NULL PRIMARY KEY, language TEXT NOT NULL DEFAULT '');
             CREATE TABLE global_asr_config (
                 id INTEGER NOT NULL PRIMARY KEY, language TEXT NOT NULL DEFAULT '');",
        )
        .await
        .expect("建旧表失败");

        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("up 失败");

        let names = table_names(&db).await;
        for table in ["role_asr_config", "global_asr_config"] {
            assert!(
                !names.contains(&table.to_string()),
                "{table} 应被删除: {names:?}"
            );
        }
    }

    /// 最关键的一条：本测试**什么都不建**，模拟「库里还没有 config 表」。
    /// 若去掉 `IF EXISTS`，这条必红（`no such table: role_asr_config`）。
    #[tokio::test]
    async fn is_idempotent_on_missing_tables() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("连接失败");
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("表不存在时 up 也必须成功");
    }
}
