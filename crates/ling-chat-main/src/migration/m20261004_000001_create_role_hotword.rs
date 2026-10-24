use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `CHECK` 与复合 `UNIQUE` 无法用 `ColumnDef` builder 表达，只能走原始 SQL。
/// `CHECK (weight BETWEEN 1 AND 50)` 只做范围兜底；离散口径 `[1, 5] ∪ {50}` 见
/// `hotword_weight.rs`。
///
/// 刻意**不写** `FOREIGN KEY`：删除角色时应用层会关闭 FK，正常路径不受影响；但 LAN 镜像
/// 在事务内应用记录，事务内的 `PRAGMA foreign_keys = OFF` 是 no-op，外键实际开启。
/// 统一不写 FK，级联清理全部落在应用层。
/// 也不额外建 `(role_id)` 索引：`UNIQUE` 的最左前缀已是 `role_id`。
const RAW_SQL: &str = "\
CREATE TABLE IF NOT EXISTS role_hotword (
    id      INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    role_id INTEGER NOT NULL,
    text    TEXT    NOT NULL,
    weight  INTEGER NOT NULL DEFAULT 4 CHECK (weight BETWEEN 1 AND 50),
    UNIQUE (role_id, text)
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
        Ok(())
    }
}

#[derive(DeriveIden)]
enum RoleHotword {
    Table,
}
