use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `CHECK` 与复合 `UNIQUE` 无法用 `ColumnDef` builder 表达，只能走原始 SQL。
///
/// 两张表都刻意**不写** `FOREIGN KEY`：无归属对象，且与 role 两表同一取舍。
/// `global_asr_config` 是**单行表**（id 恒为 0）：语言与热词要能一个事务提交，而塞进
/// `AsrSettings` 会随 `asr_set_settings` 触发 provider 重建等重副作用。
/// 注意它**没有** `role_asr_config` 那套「行存在 = 已初始化」语义——那是给 seed 用的，
/// 全局没有 seed：读路径纯读不建行（缺行 = 未配置），写路径 upsert id = 0。
const RAW_SQL: &str = "\
CREATE TABLE IF NOT EXISTS global_hotword (
    id     INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    text   TEXT    NOT NULL,
    weight INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
    lang   TEXT    NOT NULL DEFAULT '',
    UNIQUE (text, lang)
);
CREATE TABLE IF NOT EXISTS global_asr_config (
    id       INTEGER NOT NULL PRIMARY KEY,
    language TEXT    NOT NULL DEFAULT ''
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
        manager
            .drop_table(Table::drop().table(GlobalAsrConfig::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum GlobalHotword {
    Table,
}

#[derive(DeriveIden)]
enum GlobalAsrConfig {
    Table,
}
