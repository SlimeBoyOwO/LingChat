use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 两张表都刻意**不写** `FOREIGN KEY`：`library_hotword.category_id` 的级联靠应用层
/// （`replace_library` 整表重建），与本仓库其余热词表同一取舍。
/// `category_id` 可空 = 未分类：加词的摩擦不该大于现有热词表。
const RAW_SQL: &str = "\
CREATE TABLE IF NOT EXISTS hotword_category (
    id   INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name TEXT    NOT NULL,
    UNIQUE (name)
);
CREATE TABLE IF NOT EXISTS library_hotword (
    id          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER,
    text        TEXT    NOT NULL,
    weight      INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
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
            .drop_table(Table::drop().table(LibraryHotword::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(HotwordCategory::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum HotwordCategory {
    Table,
}

#[derive(DeriveIden)]
enum LibraryHotword {
    Table,
}
