use sea_orm_migration::prelude::*;

/// 创建 context_summary 表：按存档存一行上下文压缩摘要（kimi 式交接笔记）。
///
/// 与 MemoryBank 无关，独立存取；save_id 作主键，一个存档最多一行。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ContextSummary::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContextSummary::SaveId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ContextSummary::Summary).text().not_null())
                    .col(
                        ColumnDef::new(ContextSummary::CutoffCount)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContextSummary::UpdatedAt)
                            .date_time()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContextSummary::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ContextSummary {
    Table,
    SaveId,
    Summary,
    CutoffCount,
    UpdatedAt,
}
