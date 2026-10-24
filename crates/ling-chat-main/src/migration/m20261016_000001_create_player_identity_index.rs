//! 身份索引表的补建迁移：新库建表；已跑过中间版本（表已存在）的库用 IF NOT EXISTS 跳过。

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerIdentity::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerIdentity::IdentityId)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlayerIdentity::Name).text().not_null())
                    .col(
                        ColumnDef::new(PlayerIdentity::Subtitle)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(PlayerIdentity::UpdatedAt).text().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PlayerIdentity::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PlayerIdentity {
    Table,
    IdentityId,
    Name,
    Subtitle,
    UpdatedAt,
}
