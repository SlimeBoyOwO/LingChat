use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RoleIdentityRelation::Table)
                    .add_column(
                        ColumnDef::new(RoleIdentityRelation::InitialAffection)
                            .integer()
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(RoleIdentityRelation::Table)
                    .drop_column(RoleIdentityRelation::InitialAffection)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum RoleIdentityRelation {
    Table,
    InitialAffection,
}
