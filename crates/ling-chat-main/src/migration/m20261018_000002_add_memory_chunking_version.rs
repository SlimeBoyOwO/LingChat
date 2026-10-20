use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 旧记录默认没有分块版本，下次同步会按当前策略重新分块。
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE memory_document ADD COLUMN chunking_version TEXT NOT NULL DEFAULT ''",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE memory_document DROP COLUMN chunking_version")
            .await?;
        Ok(())
    }
}
