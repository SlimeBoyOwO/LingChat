use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 会话表
        manager
            .create_table(
                Table::create()
                    .table(SkillAgentConversation::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SkillAgentConversation::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SkillAgentConversation::Title).string_len(255))
                    .col(ColumnDef::new(SkillAgentConversation::ScriptKey).string_len(255))
                    .col(ColumnDef::new(SkillAgentConversation::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(SkillAgentConversation::UpdatedAt).date_time().not_null())
                    .to_owned(),
            )
            .await?;

        // 消息表
        manager
            .create_table(
                Table::create()
                    .table(SkillAgentMessage::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SkillAgentMessage::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SkillAgentMessage::ConversationId).integer().not_null())
                    .col(ColumnDef::new(SkillAgentMessage::Role).string_len(32).not_null())
                    .col(ColumnDef::new(SkillAgentMessage::Content).text())
                    .col(ColumnDef::new(SkillAgentMessage::ToolCalls).text())
                    .col(ColumnDef::new(SkillAgentMessage::ToolCallId).string_len(255))
                    .col(ColumnDef::new(SkillAgentMessage::CreatedAt).date_time().not_null())
                    .to_owned(),
            )
            .await?;

        // 消息按会话查询的索引
        manager
            .create_index(
                Index::create()
                    .name("ix_skill_agent_message_conversation")
                    .table(SkillAgentMessage::Table)
                    .col(SkillAgentMessage::ConversationId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SkillAgentMessage::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(SkillAgentConversation::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum SkillAgentConversation {
    Table,
    Id,
    Title,
    ScriptKey,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SkillAgentMessage {
    Table,
    Id,
    ConversationId,
    Role,
    Content,
    ToolCalls,
    ToolCallId,
    CreatedAt,
}
