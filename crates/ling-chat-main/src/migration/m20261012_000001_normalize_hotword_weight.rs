use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 把存量 `weight` 归一到 `[1, 5] ∪ {50}`。
///
/// **为什么必须有这条迁移**：2026-10-12 之前代码用的是连续 `clamp(1, 50)`
/// （`provider_types.rs` / `hotword_repo.rs` / 前端 `hotwordRows.ts`），
/// 用户日常输入（敲 6 / 20 / 35）已经写进了库。改口径后这些值成了非法值，
/// 而 DDL 的 `CHECK (weight BETWEEN 1 AND 50)` 是超集、拦不住它们。
///
/// **只 UPDATE，不重建表**：DDL 的 `CHECK` 保持超集作为兜底（收紧它要重建三张表，
/// 是数据丢失的高发区，收益不抵风险）。`6..=49` 取就近的合法值 5。
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
