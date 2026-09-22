use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 一个存档一条记录：这个存档用的是哪张「我的身份」卡。
        //
        // 刻意**不加外键**：`save` 表在历史上被 compat 迁移整体重建过
        // （DROP TABLE save → RENAME save_new TO save），带 FK 指向 save 的新表
        // 会挡住将来同类重建。改由应用层在删存档时顺手清理。
        //
        // 旧版本程序不认识这张表：既不写也不删，因此不影响向前兼容。
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
