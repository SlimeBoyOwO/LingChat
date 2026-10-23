use sea_orm_migration::prelude::*;

/// 旧版记忆导入后的「剧本×角色」档案目录表。
///
/// 旧版（升级前）的四段式记忆存在 `memory_bank` 表（`save_id` × `role_id` 一行一档），
/// 新版长期记忆存进每「剧本 × 角色」一张的共用归档表。用户升级后希望一键把旧版
/// 记忆整合出来，并按「剧本 × 角色」落到独立 SQL 表归档。
///
/// SQLite 的 SeaORM 无法为动态表名生成实体，因此这里只建一张**目录表**记录每个
/// 剧本×角色档案的元信息（脚本 key、角色 id/名、实际数据表名）。每个档案对应的
/// 数据表 `memory_archive_{role_id}` 与长期记忆**共用**，由
/// `ai_service::memory::store::ensure_role_archive` / `memory_archive::memory_import_legacy`
/// 在首次写入时动态创建；历史版本建的旧结构表由
/// `store::ensure_archive_table_schema` 在访问时自动补齐新列（结构与
/// `store::MEMORY_ARCHIVE_DDL` 一致）。
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
    /// 代码（实体 / 目录表读写）统一用 `memory_archive_profiles`（复数）；
    /// 必须显式指定，否则 DeriveIden 会默认建出单数 `memory_archive_profile`。
    #[sea_orm(iden = "memory_archive_profiles")]
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
