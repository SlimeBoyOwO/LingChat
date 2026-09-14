use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 对话台词嵌入向量库：按存档（save_id）持久化，用于「一键整理当前对话」。
        // 读档时把这些向量载回内存语义索引（MemoryIndex），供检索/去重。
        manager
            .create_table(
                Table::create()
                    .table(Embedding::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Embedding::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Embedding::SaveId).integer().not_null())
                    .col(ColumnDef::new(Embedding::Source).string_len(255).not_null())
                    .col(ColumnDef::new(Embedding::Text).text().not_null())
                    .col(ColumnDef::new(Embedding::Dim).integer().not_null())
                    .col(ColumnDef::new(Embedding::Vector).blob().not_null())
                    .col(ColumnDef::new(Embedding::CreatedAt).date_time().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ix_embedding_save")
                    .table(Embedding::Table)
                    .col(Embedding::SaveId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Embedding::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Embedding {
    Table,
    Id,
    SaveId,
    Source,
    Text,
    Dim,
    Vector,
    CreatedAt,
}
