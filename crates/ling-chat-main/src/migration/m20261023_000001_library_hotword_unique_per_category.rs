use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 词库唯一键从全局 `text` 改为分类内唯一：旧约束下同一个词无法放进两个分类。
///
/// SQLite 的复合唯一索引对 NULL 不生效，未分类词条另建只覆盖 `category_id IS NULL`
/// 的部分唯一索引。旧库受 `UNIQUE(text)` 限制不可能有跨分类同名词，直接搬行即可。
/// 脚本显式开事务：SQLite 的 DDL 逐条提交，中途失败会留下 `_new` 半成品。
const RAW_SQL: &str = "\
BEGIN;
CREATE TABLE library_hotword_new (
    id          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER,
    text        TEXT    NOT NULL,
    weight      INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50)
);
INSERT INTO library_hotword_new (id, category_id, text, weight)
    SELECT id, category_id, text, weight FROM library_hotword;
DROP TABLE library_hotword;
ALTER TABLE library_hotword_new RENAME TO library_hotword;
CREATE UNIQUE INDEX ux_library_hotword_category_text
    ON library_hotword (category_id, text)
    WHERE category_id IS NOT NULL;
CREATE UNIQUE INDEX ux_library_hotword_uncategorized_text
    ON library_hotword (text)
    WHERE category_id IS NULL;
COMMIT;
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(RAW_SQL).await?;
        Ok(())
    }

    /// 不可逆：回退到全局 `UNIQUE(text)` 必须丢弃跨分类的同名词。
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "m20261023_000001 不可回滚：跨分类同名词无法在全局唯一约束下保留".into(),
        ))
    }
}
