use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RoleIdentityRelation::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RoleIdentityRelation::RoleId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoleIdentityRelation::IdentityId)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoleIdentityRelation::Text)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(RoleIdentityRelation::UpdatedAt)
                            .text()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk-role_identity_relation")
                            .col(RoleIdentityRelation::RoleId)
                            .col(RoleIdentityRelation::IdentityId)
                            .primary(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(RoleIdentityRelation::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum RoleIdentityRelation {
    Table,
    RoleId,
    IdentityId,
    Text,
    UpdatedAt,
}
