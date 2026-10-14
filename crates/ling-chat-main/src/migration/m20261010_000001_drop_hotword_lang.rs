use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 三张热词表去 `lang` 列 + 唯一键去掉 `lang`。
///
/// SQLite 的 `ALTER TABLE ... DROP COLUMN` **不能删被 UNIQUE 约束引用的列**
/// （实测报 `error in table ... after drop column: no such column: lang`），故重建表。
///
/// 整段脚本自带 `BEGIN; ... COMMIT;`：SQLite 的 DDL 逐条自动提交，而 sea-orm 1.x 只给
/// Postgres 的迁移开事务（`migrator.rs` 的 `exec_with_connection`），中途失败会留下
/// `*_new` 半成品，下次启动重跑时撞名卡死。显式事务让整个重建要么全成要么全退。
///
/// ⚠️ **归并写法不要改成 `SELECT MIN(id), category_id, ..., MAX(weight) ... GROUP BY text`**：
/// 裸列取值无保证，实测 `category_id` 会取到 MAX(weight) 那一行而不是首现行。
/// 下面的 `WHERE id = (SELECT MIN(id) ...)` + 标量子查询与 GROUP BY 等价但结果确定。
///
/// 三张表的暴露面不同：`role_hotword`（分组内 `role_id` 必然同值）与 `global_hotword`
/// （无第三列）取哪行都一样；**只有 `library_hotword` 会静默挂错分类**，故它的测试最关键。
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
