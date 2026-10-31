use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MemoryDocument::Table)
                    .add_column(
                        ColumnDef::new(MemoryDocument::ChunkingVersion)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MemoryDocument::Table)
                    .drop_column(MemoryDocument::ChunkingVersion)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum MemoryDocument {
    Table,
    ChunkingVersion,
}
