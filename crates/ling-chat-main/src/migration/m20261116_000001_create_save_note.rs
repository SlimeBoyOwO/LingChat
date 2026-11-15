use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 存档级手动笔记。同一存档内所有角色共享一份，随存档删除由应用层清理。
///
/// 主键 `(save_id, id)` 的最左前缀就是按档查询的索引，不再额外建索引。
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SaveNote::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(SaveNote::SaveId).integer().not_null())
                    .col(
                        ColumnDef::new(SaveNote::Id)
                            .string()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SaveNote::Content).text().not_null())
                    .col(
                        ColumnDef::new(SaveNote::Tags)
                            .text()
                            .not_null()
                            .default("[]"),
                    )
                    .col(ColumnDef::new(SaveNote::CreatedAt).string().not_null())
                    .primary_key(
                        Index::create()
                            .col(SaveNote::SaveId)
                            .col(SaveNote::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SaveNote::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SaveNote {
    Table,
    Id,
    SaveId,
    Content,
    Tags,
    CreatedAt,
}
