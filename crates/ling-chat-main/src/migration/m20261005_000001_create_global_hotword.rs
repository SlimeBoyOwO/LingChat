use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `CHECK` 与 `UNIQUE` 无法用 `ColumnDef` builder 表达，只能走原始 SQL。
///
/// 刻意**不写** `FOREIGN KEY`：无归属对象，且与 `role_hotword` 表同一取舍。
const RAW_SQL: &str = "\
CREATE TABLE IF NOT EXISTS global_hotword (
    id     INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    text   TEXT    NOT NULL,
    weight INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
    UNIQUE (text)
);";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(RAW_SQL).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(GlobalHotword::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum GlobalHotword {
    Table,
}
