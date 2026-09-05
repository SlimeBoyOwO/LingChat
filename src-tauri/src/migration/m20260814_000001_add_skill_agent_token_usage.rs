use sea_orm::Statement;
use sea_orm_migration::prelude::*;

/// 为 skill_agent_message 增加 prompt_tokens / completion_tokens 列（AI 助手 token 用量统计）。
///
/// 稳定性说明：只加可空列，SQLite 元数据级操作，不动已有行/索引；
/// 旧库升级后旧行两列自动为 NULL，前端按可选字段读取。
/// 用法统计只落 assistant 消息，因此没有在 conversation 表聚合列。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 防御性预检：SQLite 的 ADD COLUMN 没有 IF NOT EXISTS 语法，先查
        // pragma_table_info 确认列不存在再执行，防 seaql_migrations 记录
        // 异常（如被手动清空）导致重复执行时报「duplicate column」。
        for (column, kind) in [
            ("prompt_tokens", "prompt_tokens"),
            ("completion_tokens", "completion_tokens"),
        ] {
            let rows = manager
                .get_connection()
                .query_all(Statement::from_string(
                    manager.get_database_backend(),
                    format!(
                        "SELECT name FROM pragma_table_info('skill_agent_message') WHERE name = '{kind}'"
                    ),
                ))
                .await?;
            if rows.is_empty() {
                let col = match column {
                    "prompt_tokens" => SkillAgentMessage::PromptTokens,
                    _ => SkillAgentMessage::CompletionTokens,
                };
                manager
                    .alter_table(
                        Table::alter()
                            .table(SkillAgentMessage::Table)
                            .add_column(ColumnDef::new(col).integer().null())
                            .to_owned(),
                    )
                    .await?;
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(SkillAgentMessage::Table)
                    .drop_column(SkillAgentMessage::PromptTokens)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(SkillAgentMessage::Table)
                    .drop_column(SkillAgentMessage::CompletionTokens)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SkillAgentMessage {
    Table,
    PromptTokens,
    CompletionTokens,
}
