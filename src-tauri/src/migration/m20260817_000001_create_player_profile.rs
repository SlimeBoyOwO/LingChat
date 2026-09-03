use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

/// 玩家人设卡表（纯 DB 多卡存储）。
///
/// 历史：本分支早期曾创建 `player_profile` 表保存单张玩家档案；随后一度改为
/// 文件驱动（`game_data/player/settings.yml`），并把本迁移降级为 no-op 占位，
/// 以免老库 `seaql_migrations` 已记录该版本导致启动报错。
///
/// Owner 最终决策：玩家配置回归**纯 DB**（不接受文件系统版本/迁移框架），
/// 且玩家支持**多张人设卡并存**。因此这里把本迁移恢复为真实建表：
/// - 一行 = 一张玩家人设卡（`card_id` 为主键，沿用原目录名语义，可读且稳定）；
/// - `is_active` 标记当前激活人设，用**部分唯一索引**保证全表至多一个激活行；
/// - 头像功能按 Owner 决策移除，不再存任何头像字段。
///
/// 注意：仅本分支早期开发库可能已把本迁移作为 no-op 记录过（表不存在），
/// 升级后首次查询会报「表不存在」；此类开发库可直接删除/重建
/// `game_database.db`。正式发布库从未记录过本版本，会正常建表。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlayerProfile::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlayerProfile::CardId)
                            .string_len(128)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PlayerProfile::UserName).string_len(255).not_null())
                    .col(ColumnDef::new(PlayerProfile::UserSubtitle).string_len(255))
                    .col(ColumnDef::new(PlayerProfile::UserPrompt).text())
                    .col(ColumnDef::new(PlayerProfile::Info).text())
                    .col(ColumnDef::new(PlayerProfile::SystemPromptExample).text())
                    .col(
                        ColumnDef::new(PlayerProfile::IsActive)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        // 部分唯一索引：只约束 is_active = 1 的行，保证「至多一个激活人设」，
        // 即使并发切换也不会出现两个激活卡。sea-query 不支持部分索引，用原生 SQL。
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "CREATE UNIQUE INDEX IF NOT EXISTS uq_player_profile_active \
                 ON player_profile (is_active) WHERE is_active = 1",
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 回滚时若表还在就删掉（SQLite 删表会连带删除其索引），不存在则静默通过。
        manager
            .drop_table(
                Table::drop()
                    .table(PlayerProfile::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum PlayerProfile {
    Table,
    CardId,
    UserName,
    UserSubtitle,
    UserPrompt,
    Info,
    SystemPromptExample,
    IsActive,
}
