use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `CHECK` 与复合 `UNIQUE` 无法用 `ColumnDef` builder 表达，只能走原始 SQL。
///
/// 两张表都刻意**不写** `FOREIGN KEY`（spec §3.6.2）：本仓库这类外键不提供级联保障
/// （删除靠手工清理，且那时 FK 已关闭），却会让旧版二进制 / 旧设备在 `DELETE FROM role` 时失败。
/// 也不额外建 `(role_id)` 索引：`UNIQUE` 的最左前缀已是 `role_id`。
const RAW_SQL: &str = "\
CREATE TABLE IF NOT EXISTS role_asr_config (
    role_id  INTEGER NOT NULL PRIMARY KEY,
    language TEXT    NOT NULL DEFAULT ''
);
CREATE TABLE IF NOT EXISTS role_hotword (
    id      INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    role_id INTEGER NOT NULL,
    text    TEXT    NOT NULL,
    weight  INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
    lang    TEXT    NOT NULL DEFAULT '',
    UNIQUE (role_id, text, lang)
);";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(RAW_SQL).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RoleHotword::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RoleAsrConfig::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum RoleAsrConfig {
    Table,
}

#[derive(DeriveIden)]
enum RoleHotword {
    Table,
}
