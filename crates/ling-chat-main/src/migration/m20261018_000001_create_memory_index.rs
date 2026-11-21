use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                    .check(Expr::col(MemoryDocument::Kind).is_in(["raw", "diary", "note"]))
                    .check(
                        Expr::col(MemoryDocument::Kind)
                            .eq("note")
                            .and(Expr::col(MemoryDocument::SaveId).is_null())
                            .or(Expr::col(MemoryDocument::Kind)
                                .ne("note")
                                .and(Expr::col(MemoryDocument::SaveId).is_not_null())),
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
                    .col(ColumnDef::new(MemoryChunk::Ordinal).integer().not_null())
                    .col(ColumnDef::new(MemoryChunk::Version).text().not_null())
                    .col(ColumnDef::new(MemoryChunk::Payload).text().not_null())
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
                    .table(MemoryEmbedding::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryEmbedding::ChunkId)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MemoryEmbedding::ChunkVersion)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEmbedding::ModelVersion)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryEmbedding::Payload).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(MemoryEmbedding::Table, MemoryEmbedding::ChunkId)
                            .to(MemoryChunk::Table, MemoryChunk::Id)
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
                    .col(
                        ColumnDef::new(MemoryIndexState::Generation)
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
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MemoryIndexState::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MemoryEmbedding::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MemoryChunk::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MemoryDocument::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
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
}

#[derive(DeriveIden)]
enum MemoryChunk {
    Table,
    Id,
    DocumentId,
    Ordinal,
    Version,
    Payload,
}

#[derive(DeriveIden)]
enum MemoryEmbedding {
    Table,
    ChunkId,
    ChunkVersion,
    ModelVersion,
    Payload,
}

#[derive(DeriveIden)]
enum MemoryIndexState {
    Table,
    SaveId,
    RoleId,
    Generation,
    ScopeKey,
    IndexVersion,
}
