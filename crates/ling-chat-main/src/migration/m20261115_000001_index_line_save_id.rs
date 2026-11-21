use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// 台词表按存档查询的索引。
///
/// `line` 是所有存档共用的一张单表，建表时只声明了 `save_id` 外键，而 SQLite 不会为
/// 外键自动建索引。每次存档都要先读回本档的全部台词再逐行差分，于是变成全表扫描，
/// 存档累积越多、手动存档就越慢。
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("ix_line_save_id")
                    .table(Line::Table)
                    .col(Line::SaveId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("ix_line_save_id").if_exists().to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Line {
    Table,
    SaveId,
}
