use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 把存量 `weight` 归一到 `[1, 5] ∪ {50}`：旧代码允许连续值，库里已有 6..=49。
///
/// 只 UPDATE 不重建表：DDL 的 `CHECK` 保持超集兜底，`6..=49` 就近降为 5。
const UP_SQL: &str = "\
UPDATE role_hotword    SET weight = 5 WHERE weight BETWEEN 6 AND 49;
UPDATE global_hotword  SET weight = 5 WHERE weight BETWEEN 6 AND 49;
UPDATE library_hotword SET weight = 5 WHERE weight BETWEEN 6 AND 49;";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(UP_SQL).await?;
        Ok(())
    }

    /// 不可逆：`6..=49` 与「本来就是 5」区分不开，下调无法还原。
    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
