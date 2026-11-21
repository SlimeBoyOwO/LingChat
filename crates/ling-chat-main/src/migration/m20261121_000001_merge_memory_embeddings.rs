use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{DatabaseTransaction, Statement, TransactionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;
        if has_merged_embeddings(&transaction).await? {
            return transaction.commit().await;
        }
        // 只有与当前分块匹配的旧向量可以搬过去，旧索引版本由下次同步重新记录。
        transaction
            .execute_unprepared(
                "ALTER TABLE memory_chunk ADD COLUMN embedding TEXT;
                 UPDATE memory_chunk SET embedding = (
                     SELECT payload FROM memory_embedding
                     WHERE chunk_id = memory_chunk.id AND chunk_version = memory_chunk.version
                 );
                 DROP TABLE memory_embedding;
                 DELETE FROM memory_index_state;
                 ALTER TABLE memory_index_state DROP COLUMN generation;",
            )
            .await?;
        transaction.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let transaction = manager.get_connection().begin().await?;
        if !has_merged_embeddings(&transaction).await? {
            return transaction.commit().await;
        }
        // 回退后向量可从现有分块重建，这里只恢复旧表结构。
        transaction
            .execute_unprepared(
                "ALTER TABLE memory_chunk DROP COLUMN embedding;
                 CREATE TABLE memory_embedding (
                     chunk_id TEXT NOT NULL PRIMARY KEY REFERENCES memory_chunk(id) ON DELETE CASCADE,
                     chunk_version TEXT NOT NULL,
                     model_version TEXT NOT NULL,
                     payload TEXT NOT NULL
                 );
                 DELETE FROM memory_index_state;
                 ALTER TABLE memory_index_state ADD COLUMN generation INTEGER NOT NULL DEFAULT 0;",
            )
            .await?;
        transaction.commit().await
    }
}

async fn has_merged_embeddings(transaction: &DatabaseTransaction) -> Result<bool, DbErr> {
    // SQLite 的表结构先于迁移记录提交，重试时通过表和字段判断上一次是否已完成。
    let schema = transaction
        .query_one(Statement::from_string(
            transaction.get_database_backend(),
            "SELECT
                EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'memory_chunk') AS chunk_table,
                EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'memory_index_state') AS state_table,
                EXISTS(SELECT 1 FROM pragma_table_info('memory_chunk') WHERE name = 'embedding') AS embedding_column,
                EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'memory_embedding') AS embedding_table,
                EXISTS(SELECT 1 FROM pragma_table_info('memory_index_state') WHERE name = 'generation') AS generation_column"
                .to_owned(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("无法读取记忆索引表结构".into()))?;
    if !schema.try_get::<bool>("", "chunk_table")? || !schema.try_get::<bool>("", "state_table")? {
        return Err(DbErr::Custom("缺少记忆索引表，无法迁移向量".into()));
    }
    let embedding_column: bool = schema.try_get("", "embedding_column")?;
    let embedding_table: bool = schema.try_get("", "embedding_table")?;
    let generation_column: bool = schema.try_get("", "generation_column")?;
    match (embedding_column, embedding_table, generation_column) {
        (true, false, false) => Ok(true),
        (false, true, true) => Ok(false),
        _ => Err(DbErr::Custom("记忆索引表结构不完整，无法迁移向量".into())),
    }
}
