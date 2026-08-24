use sea_orm::Statement;
use sea_orm_migration::prelude::*;

/// 为 skill_agent_message 增加 cached_tokens 列（AI 助手缓存命中统计）。
///
/// 稳定性说明：只加可空列，SQLite 元数据级操作，不动已有行/索引；
/// 旧库升级后旧行该列自动为 NULL，前端按可选字段读取（无缓存数据不显示命中率）。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 防御性预检：SQLite 的 ADD COLUMN 没有 IF NOT EXISTS 语法，先查
        // pragma_table_info 确认列不存在再执行，防 seaql_migrations 记录
        // 异常（如被手动清空）导致重复执行时报「duplicate column」。
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT name FROM pragma_table_info('skill_agent_message') WHERE name = 'cached_tokens'"
                    .to_string(),
            ))
            .await?;
        if rows.is_empty() {
            manager
                .alter_table(
                    Table::alter()
                        .table(SkillAgentMessage::Table)
                        .add_column(ColumnDef::new(SkillAgentMessage::CachedTokens).integer().null())
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SkillAgentMessage::Table)
                    .drop_column(SkillAgentMessage::CachedTokens)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SkillAgentMessage {
    Table,
    CachedTokens,
}
