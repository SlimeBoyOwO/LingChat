use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Line::Table)
                    .add_column(ColumnDef::new(Line::Thinking).text().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Line::Table)
                    .drop_column(Line::Thinking)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Line {
    Table,
    Thinking,
}
