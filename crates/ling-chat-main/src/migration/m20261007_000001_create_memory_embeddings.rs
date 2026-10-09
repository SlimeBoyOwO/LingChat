use sea_orm_migration::prelude::*;

/// 向量记忆表：长期记忆（memory_store 写入，用户新消息轮 RAG 检索 / memory_delete 删除）。
///
/// 每条记忆按固定长度切块后逐块入库，同一记忆的多个分块共享 `memory_id`。
/// `embedding` 存 L2 归一化的 Float32 小端字节，检索时在内存里算余弦相似度。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MemoryEmbedding::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryEmbedding::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(MemoryEmbedding::RoleId).integer().not_null())
                    .col(
                        ColumnDef::new(MemoryEmbedding::MemoryId)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEmbedding::ChunkIndex)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MemoryEmbedding::Content).text().not_null())
                    .col(ColumnDef::new(MemoryEmbedding::Tags).text().null())
                    .col(ColumnDef::new(MemoryEmbedding::Embedding).blob().not_null())
                    .col(ColumnDef::new(MemoryEmbedding::Dim).integer().not_null())
                    .col(
                        ColumnDef::new(MemoryEmbedding::Model)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryEmbedding::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_memory_embeddings_role")
                    .table(MemoryEmbedding::Table)
                    .col(MemoryEmbedding::RoleId)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MemoryEmbedding::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MemoryEmbedding {
    /// 代码（实体 / 存储层 / LAN 同步）统一读写 `memory_embeddings`（复数）；
    /// 必须显式指定，否则 DeriveIden 会默认建出单数 `memory_embedding`。
    #[sea_orm(iden = "memory_embeddings")]
    Table,
    Id,
    RoleId,
    MemoryId,
    ChunkIndex,
    Content,
    Tags,
    Embedding,
    Dim,
    Model,
    CreatedAt,
}
