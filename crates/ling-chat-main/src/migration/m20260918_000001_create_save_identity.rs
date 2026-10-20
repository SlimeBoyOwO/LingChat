use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SaveIdentity::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SaveIdentity::SaveId)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SaveIdentity::IdentityId).text().not_null())
                    .col(ColumnDef::new(SaveIdentity::CreatedAt).text().not_null())
                    .col(ColumnDef::new(SaveIdentity::UpdatedAt).text().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SaveIdentity::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SaveIdentity {
    Table,
    SaveId,
    IdentityId,
    CreatedAt,
    UpdatedAt,
}
