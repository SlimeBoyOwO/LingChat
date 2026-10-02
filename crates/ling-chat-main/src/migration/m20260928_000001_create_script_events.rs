use sea_orm_migration::prelude::*;

/// 剧作事件流水：审计用、不是权威状态（权威状态永远在磁盘），删掉整张表不影响任何判定。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ScriptEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ScriptEvents::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ScriptEvents::ConversationId).integer())
                    .col(ColumnDef::new(ScriptEvents::ScriptKey).string_len(255))
                    .col(ColumnDef::new(ScriptEvents::Kind).string_len(64).not_null())
                    .col(ColumnDef::new(ScriptEvents::Target).string_len(255))
                    .col(ColumnDef::new(ScriptEvents::Detail).text())
                    // Unix 秒。用整数而不是 datetime：这张表只按时间排序，不需要时区语义。
                    .col(
                        ColumnDef::new(ScriptEvents::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_script_events_key_id")
                    .table(ScriptEvents::Table)
                    .col(ScriptEvents::ScriptKey)
                    .col(ScriptEvents::Id)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ScriptEvents::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ScriptEvents {
    Table,
    Id,
    ConversationId,
    ScriptKey,
    Kind,
    Target,
    Detail,
    CreatedAt,
}
