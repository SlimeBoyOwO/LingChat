use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 删除两张识别语言表（识别语言功能已废弃）。
///
/// 必须 `IF EXISTS`：这两张表由更早的迁移创建，重放或部分重放时存在性不确定。
/// 脚本自带事务：sea-orm 1.x 不为 SQLite 迁移开事务，失败会留下半成品。
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
