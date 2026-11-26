use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 存档信纸：正文存在 data/letters/{id}.md，这里只记归属与页序。
/// 一张信纸只属于一个存档，另存到别的档时复制成新 id。
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SaveLetter::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SaveLetter::Id)
                            .string()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SaveLetter::SaveId).integer().not_null())
                    .col(ColumnDef::new(SaveLetter::Page).integer().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_save_letter_save_id")
                    .table(SaveLetter::Table)
                    .col(SaveLetter::SaveId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SaveLetter::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SaveLetter {
    Table,
    Id,
    SaveId,
    Page,
}
