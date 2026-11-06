use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 三张热词表去掉 `lang` 列，唯一键同步去掉 `lang`。
///
/// SQLite 不能删被 UNIQUE 约束引用的列，故重建表。脚本自带事务：sea-orm 1.x 不为
/// SQLite 迁移开事务，中途失败会留下 `_new` 半成品。归并用 `WHERE id = (SELECT MIN(id) ...)`
/// 取首现行，不要改成裸列 GROUP BY（裸列取值不确定）。
const RAW_SQL: &str = "\
BEGIN;
CREATE TABLE role_hotword_new (
    id      INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    role_id INTEGER NOT NULL,
    text    TEXT    NOT NULL,
    weight  INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
    UNIQUE (role_id, text)
);
INSERT INTO role_hotword_new (id, role_id, text, weight)
    SELECT h.id, h.role_id, h.text,
           (SELECT MAX(weight) FROM role_hotword w
             WHERE w.role_id = h.role_id AND w.text = h.text)
      FROM role_hotword h
     WHERE h.id = (SELECT MIN(id) FROM role_hotword m
                    WHERE m.role_id = h.role_id AND m.text = h.text);
DROP TABLE role_hotword;
ALTER TABLE role_hotword_new RENAME TO role_hotword;

CREATE TABLE global_hotword_new (
    id     INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    text   TEXT    NOT NULL,
    weight INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
    UNIQUE (text)
);
INSERT INTO global_hotword_new (id, text, weight)
    SELECT h.id, h.text,
           (SELECT MAX(weight) FROM global_hotword w WHERE w.text = h.text)
      FROM global_hotword h
     WHERE h.id = (SELECT MIN(id) FROM global_hotword m WHERE m.text = h.text);
DROP TABLE global_hotword;
ALTER TABLE global_hotword_new RENAME TO global_hotword;

CREATE TABLE library_hotword_new (
    id          INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    category_id INTEGER,
    text        TEXT    NOT NULL,
    weight      INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
    UNIQUE (text)
);
INSERT INTO library_hotword_new (id, category_id, text, weight)
    SELECT h.id, h.category_id, h.text,
           (SELECT MAX(weight) FROM library_hotword w WHERE w.text = h.text)
      FROM library_hotword h
     WHERE h.id = (SELECT MIN(id) FROM library_hotword m WHERE m.text = h.text);
DROP TABLE library_hotword;
ALTER TABLE library_hotword_new RENAME TO library_hotword;
COMMIT;
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(RAW_SQL).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 信息已丢失（同词异语言被归并），无法还原。
        Err(DbErr::Migration(
            "m20261010_000001 不可回滚：归并已丢弃同词异语言行".into(),
        ))
    }
}
