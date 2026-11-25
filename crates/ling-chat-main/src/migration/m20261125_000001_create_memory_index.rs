use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
};
use sea_orm_migration::seaql_migrations;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;
        let manager = SchemaManager::new(&transaction);
        manager
            .create_table(
                Table::create()
                    .table(MemoryDocument::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryDocument::Id)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MemoryDocument::RoleId).integer().not_null())
                    .col(ColumnDef::new(MemoryDocument::SaveId).integer())
                    .col(ColumnDef::new(MemoryDocument::Kind).text().not_null())
                    .col(ColumnDef::new(MemoryDocument::Version).text().not_null())
                    .col(ColumnDef::new(MemoryDocument::Payload).text().not_null())
                    .col(
                        ColumnDef::new(MemoryDocument::ChunkingVersion)
                            .text()
                            .not_null(),
                    )
                    .check(Expr::col(MemoryDocument::Kind).is_in(["raw", "diary", "note"]))
                    .check(
                        Expr::col(MemoryDocument::Kind)
                            .eq("note")
                            .or(Expr::col(MemoryDocument::SaveId).is_not_null()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MemoryDocument::Table, MemoryDocument::RoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MemoryDocument::Table, MemoryDocument::SaveId)
                            .to(Save::Table, Save::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ix_memory_document_scope")
                    .table(MemoryDocument::Table)
                    .col(MemoryDocument::RoleId)
                    .col(MemoryDocument::SaveId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MemoryChunk::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryChunk::Id)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MemoryChunk::DocumentId).text().not_null())
                    .col(
                        ColumnDef::new(MemoryChunk::Ordinal)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryChunk::Version).text().not_null())
                    .col(
                        ColumnDef::new(MemoryChunk::StartByte)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryChunk::EndByte)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryChunk::Embedding).text())
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
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MemoryIndexState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryIndexState::SaveId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryIndexState::RoleId)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryIndexState::ScopeKey).text().not_null())
                    .col(
                        ColumnDef::new(MemoryIndexState::IndexVersion)
                            .text()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(MemoryIndexState::SaveId)
                            .col(MemoryIndexState::RoleId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MemoryIndexState::Table, MemoryIndexState::SaveId)
                            .to(Save::Table, Save::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MemoryIndexState::Table, MemoryIndexState::RoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;
        drop_index_tables(&SchemaManager::new(&transaction)).await?;
        transaction.commit().await
    }
}

pub(super) async fn reset_preview_index(db: &DatabaseConnection) -> Result<(), DbErr> {
    const PREVIEW_MIGRATIONS: [&str; 4] = [
        "m20261018_000001_create_memory_index",
        "m20261018_000002_add_memory_chunking_version",
        "m20261113_000001_memory_chunk_ranges",
        "m20261121_000001_merge_memory_embeddings",
    ];
    let transaction = db.begin().await?;
    let obsolete = seaql_migrations::Entity::delete_many()
        .filter(seaql_migrations::Column::Version.is_in(PREVIEW_MIGRATIONS))
        .exec(&transaction)
        .await?;
    if obsolete.rows_affected > 0 {
        // 旧版 PR 只清理可重建的索引，并与迁移记录一起提交。
        let manager = SchemaManager::new(&transaction);
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("memory_embedding"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        drop_index_tables(&manager).await?;
    }
    transaction.commit().await
}

async fn drop_index_tables(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for table in [
        MemoryIndexState::Table.into_iden(),
        MemoryChunk::Table.into_iden(),
        MemoryDocument::Table.into_iden(),
    ] {
        manager
            .drop_table(Table::drop().table(table).if_exists().to_owned())
            .await?;
    }
    Ok(())
}

#[derive(DeriveIden)]
enum Role {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Save {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum MemoryDocument {
    Table,
    Id,
    RoleId,
    SaveId,
    Kind,
    Version,
    Payload,
    ChunkingVersion,
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
    Embedding,
}

#[derive(DeriveIden)]
enum MemoryIndexState {
    Table,
    SaveId,
    RoleId,
    ScopeKey,
    IndexVersion,
}
