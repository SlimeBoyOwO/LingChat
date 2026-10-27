use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 删掉两张识别语言表（「识别语言」功能整体废弃，见计划 2026-10-11）。
///
/// ⚠️ 必须 `IF EXISTS`：`role_asr_config` / `global_asr_config` 由更早的
/// m20261004 / m20261005 建（它们当初与热词同表落库），故重放时这两张表**已经存在**；
/// 而不完整重放（只跑到本条）或将来删除建表迁移时它们又**不存在**。
/// 漏掉 `IF EXISTS`，前一种情况迁移直接失败、库卡在半途。
///
/// 整段自带 `BEGIN; ... COMMIT;`：与 `m20261010_000001` 同一取舍——sea-orm 1.x 只给
/// Postgres 的迁移开事务（`migrator.rs` 的 `exec_with_connection`），SQLite 走裸执行，
/// 失败会留下半成品。
const RAW_SQL: &str = "\
BEGIN;
DROP TABLE IF EXISTS role_asr_config;
DROP TABLE IF EXISTS global_asr_config;
COMMIT;
";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.get_connection().execute_unprepared(RAW_SQL).await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 数据已丢弃，无法还原：与 m20261010_000001 一致只做 no-op 提示。
        Err(DbErr::Migration(
            "m20261011_000001 不可回滚：识别语言数据已删除".into(),
        ))
    }
}
