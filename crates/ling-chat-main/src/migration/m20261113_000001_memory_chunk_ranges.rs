//! 分块仅保存正文中的 UTF-8 字节范围，完整正文仍由记忆文档保存。
//! 分块和向量均可从文档重建；升级与回退只清空这些派生缓存，不修改聊天或笔记。

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::TransactionTrait;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        replace_chunk_table(manager, true).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        replace_chunk_table(manager, false).await
    }
}

async fn replace_chunk_table(manager: &SchemaManager<'_>, use_ranges: bool) -> Result<(), DbErr> {
    let transaction = manager.get_connection().begin().await?;
    let manager = SchemaManager::new(&transaction);
    manager
        .exec_stmt(
            Query::delete()
                .from_table(MemoryEmbedding::Table)
                .to_owned(),
        )
        .await?;
    manager
        .exec_stmt(
            Query::delete()
                .from_table(MemoryIndexState::Table)
                .to_owned(),
        )
        .await?;
    manager
        .drop_table(Table::drop().table(MemoryChunk::Table).to_owned())
        .await?;

    let mut table = Table::create();
    table
        .table(MemoryChunk::Table)
        .col(
            ColumnDef::new(MemoryChunk::Id)
                .text()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(MemoryChunk::DocumentId).text().not_null())
        .col(ColumnDef::new(MemoryChunk::Ordinal).integer().not_null())
        .col(ColumnDef::new(MemoryChunk::Version).text().not_null())
        .index(
            Index::create()
                .unique()
                .col(MemoryChunk::DocumentId)
                .col(MemoryChunk::Ordinal),
        )
        .foreign_key(
            ForeignKey::create()
                .from(MemoryChunk::Table, MemoryChunk::DocumentId)
                .to(MemoryDocument::Table, MemoryDocument::Id)
                .on_delete(ForeignKeyAction::Cascade),
        );
    if use_ranges {
        table
            .col(
                ColumnDef::new(MemoryChunk::StartByte)
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(MemoryChunk::EndByte)
                    .big_integer()
                    .not_null(),
            );
    } else {
        table.col(ColumnDef::new(MemoryChunk::Payload).text().not_null());
    }
    manager.create_table(table).await?;
    transaction.commit().await
}

#[derive(DeriveIden)]
enum MemoryDocument {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum MemoryChunk {
    Table,
    Id,
    DocumentId,
    Ordinal,
    Version,
    StartByte,
    EndByte,
    Payload,
}

#[derive(DeriveIden)]
enum MemoryEmbedding {
    Table,
}

#[derive(DeriveIden)]
enum MemoryIndexState {
    Table,
}
