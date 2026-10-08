use sea_orm_migration::prelude::*;

/// 旧版记忆导入后的「剧本×角色」档案目录表。
///
/// 旧版（升级前）的四段式记忆存在 `memory_bank` 表（`save_id` × `role_id` 一行一档），
/// 新记忆系统 `memory_embeddings` 只按 `role_id` 隔离、不分存档。用户升级后希望
/// 一键把旧版记忆整合出来，并按「剧本 × 角色」落到独立 SQL 表归档。
///
/// SQLite 的 SeaORM 无法为动态表名生成实体，因此这里只建一张**目录表**记录每个
/// 剧本×角色档案的元信息（脚本 key、角色 id/名、实际数据表名）。每个档案对应的
/// 数据表 `memory_archive_{id}` 由 `api::memory::memory_import_legacy` 在首次导入时
/// 动态创建，结构见该命令的注释。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MemoryArchiveProfile::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::ScriptKey)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::ScriptRoleKey)
                            .string()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::RoleId)
                            .integer()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::RoleName)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::TableName)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::CreatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MemoryArchiveProfile::UpdatedAt)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MemoryArchiveProfile::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum MemoryArchiveProfile {
    Table,
    Id,
    ScriptKey,
    ScriptRoleKey,
    RoleId,
    RoleName,
    TableName,
    CreatedAt,
    UpdatedAt,
}
