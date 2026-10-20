use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 文档保存来源，分块保存检索正文，向量独立存储以便模型变化后重建。
        for sql in [
            "CREATE TABLE memory_document (
                id TEXT PRIMARY KEY NOT NULL,
                role_id INTEGER NOT NULL REFERENCES role(id) ON DELETE CASCADE,
                save_id INTEGER REFERENCES save(id) ON DELETE CASCADE,
                kind TEXT NOT NULL CHECK (kind IN ('raw','diary','note')),
                version TEXT NOT NULL, payload TEXT NOT NULL,
                CHECK ((kind='note' AND save_id IS NULL) OR (kind!='note' AND save_id IS NOT NULL))
            )",
            "CREATE INDEX ix_memory_document_scope ON memory_document(role_id,save_id)",
            "CREATE TABLE memory_chunk (
                id TEXT PRIMARY KEY NOT NULL,
                document_id TEXT NOT NULL REFERENCES memory_document(id) ON DELETE CASCADE,
                ordinal INTEGER NOT NULL, version TEXT NOT NULL, payload TEXT NOT NULL,
                UNIQUE(document_id,ordinal)
            )",
            "CREATE TABLE memory_embedding (
                chunk_id TEXT PRIMARY KEY NOT NULL REFERENCES memory_chunk(id) ON DELETE CASCADE,
                chunk_version TEXT NOT NULL, model_version TEXT NOT NULL, payload TEXT NOT NULL
            )",
            // 每个存档与角色保留独立版本，防止并发任务互相覆盖索引状态。
            "CREATE TABLE memory_index_state (
                save_id INTEGER NOT NULL REFERENCES save(id) ON DELETE CASCADE,
                role_id INTEGER NOT NULL REFERENCES role(id) ON DELETE CASCADE,
                generation INTEGER NOT NULL, scope_key TEXT NOT NULL, index_version TEXT NOT NULL,
                PRIMARY KEY(save_id,role_id)
            )",
        ] {
            manager.get_connection().execute_unprepared(sql).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "memory_index_state",
            "memory_embedding",
            "memory_chunk",
            "memory_document",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        Ok(())
    }
}
