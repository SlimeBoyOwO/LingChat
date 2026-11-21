use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, TransactionTrait};
use sea_orm_migration::prelude::*;

use crate::db::entities::agent_conversation;

/// 剧本编辑器助手会话表升级为通用 Agent 会话表，并支持绑定存档的导演会话。
///
/// `skill_agent_conversation` / `skill_agent_message` 改名为 `agent_conversation` /
/// `agent_message`，加 `kind`（skill | director）与 `save_id`。改名与加列在同一个
/// 事务里完成：sea-orm 1.x 不把迁移语句包进 SQLite 事务，中途失败会留下半改名结构。
///
/// 有导演会话时不可回滚：回退会连带抹掉这些会话与消息。
#[derive(DeriveMigrationName)]
pub struct Migration;

const MESSAGE_INDEX: &str = "ix_agent_message_conversation";
const OLD_MESSAGE_INDEX: &str = "ix_skill_agent_message_conversation";
const DIRECTOR_INDEX: &str = "ix_agent_conversation_director";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        let result = {
            let schema = SchemaManager::new(&txn);
            apply(&schema).await
        };
        match result {
            Ok(()) => txn.commit().await?,
            Err(e) => {
                let _ = txn.rollback().await;
                return Err(e);
            },
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let directors = agent_conversation::Entity::find()
            .filter(agent_conversation::Column::Kind.eq(KIND_DIRECTOR))
            .count(manager.get_connection())
            .await?;
        if directors > 0 {
            return Err(DbErr::Migration(format!(
                "m20261115_000001 不可回滚：已有 {directors} 条导演会话，回退会连带删掉它们"
            )));
        }

        let txn = manager.get_connection().begin().await?;
        let result = {
            let schema = SchemaManager::new(&txn);
            revert(&schema).await
        };
        match result {
            Ok(()) => txn.commit().await?,
            Err(e) => {
                let _ = txn.rollback().await;
                return Err(e);
            },
        }
        Ok(())
    }
}

async fn apply(schema: &SchemaManager<'_>) -> Result<(), DbErr> {
    schema
        .rename_table(
            Table::rename()
                .table(SkillAgentConversation::Table, AgentConversation::Table)
                .to_owned(),
        )
        .await?;
    schema
        .rename_table(
            Table::rename()
                .table(SkillAgentMessage::Table, AgentMessage::Table)
                .to_owned(),
        )
        .await?;

    // SQLite 没有 ALTER INDEX，索引删掉按新表名重建
    schema
        .drop_index(
            Index::drop()
                .name(OLD_MESSAGE_INDEX)
                .table(AgentMessage::Table)
                .to_owned(),
        )
        .await?;
    schema
        .create_index(
            Index::create()
                .name(MESSAGE_INDEX)
                .table(AgentMessage::Table)
                .col(AgentMessage::ConversationId)
                .to_owned(),
        )
        .await?;

    schema
        .alter_table(
            Table::alter()
                .table(AgentConversation::Table)
                .add_column(
                    ColumnDef::new(AgentConversation::Kind)
                        .text()
                        .not_null()
                        .default(KIND_SKILL),
                )
                .to_owned(),
        )
        .await?;
    schema
        .alter_table(
            Table::alter()
                .table(AgentConversation::Table)
                .add_column(ColumnDef::new(AgentConversation::SaveId).integer())
                .to_owned(),
        )
        .await?;

    // updated_at 进索引，否则「取该存档最新导演会话」仍要走临时排序
    schema
        .create_index(
            Index::create()
                .name(DIRECTOR_INDEX)
                .table(AgentConversation::Table)
                .col(AgentConversation::Kind)
                .col(AgentConversation::SaveId)
                .col(AgentConversation::UpdatedAt)
                .to_owned(),
        )
        .await?;

    Ok(())
}

async fn revert(schema: &SchemaManager<'_>) -> Result<(), DbErr> {
    // 删列在删索引之后：SQLite 不允许删被索引引用的列
    schema
        .drop_index(
            Index::drop()
                .name(DIRECTOR_INDEX)
                .table(AgentConversation::Table)
                .to_owned(),
        )
        .await?;
    schema
        .alter_table(
            Table::alter()
                .table(AgentConversation::Table)
                .drop_column(AgentConversation::SaveId)
                .to_owned(),
        )
        .await?;
    schema
        .alter_table(
            Table::alter()
                .table(AgentConversation::Table)
                .drop_column(AgentConversation::Kind)
                .to_owned(),
        )
        .await?;

    schema
        .drop_index(
            Index::drop()
                .name(MESSAGE_INDEX)
                .table(AgentMessage::Table)
                .to_owned(),
        )
        .await?;
    schema
        .rename_table(
            Table::rename()
                .table(AgentConversation::Table, SkillAgentConversation::Table)
                .to_owned(),
        )
        .await?;
    schema
        .rename_table(
            Table::rename()
                .table(AgentMessage::Table, SkillAgentMessage::Table)
                .to_owned(),
        )
        .await?;
    schema
        .create_index(
            Index::create()
                .name(OLD_MESSAGE_INDEX)
                .table(SkillAgentMessage::Table)
                .col(SkillAgentMessage::ConversationId)
                .to_owned(),
        )
        .await?;

    Ok(())
}

const KIND_SKILL: &str = "skill";
const KIND_DIRECTOR: &str = "director";

#[derive(DeriveIden)]
enum SkillAgentConversation {
    Table,
}

#[derive(DeriveIden)]
enum SkillAgentMessage {
    Table,
    ConversationId,
}

#[derive(DeriveIden)]
enum AgentConversation {
    Table,
    Kind,
    SaveId,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum AgentMessage {
    Table,
    ConversationId,
}
