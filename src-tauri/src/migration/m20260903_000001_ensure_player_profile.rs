use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

/// 幂等自愈：确保 `player_profile` 表与单激活索引一定存在。
///
/// 历史背景：本分支早期开发版把 `m20260817_000001` 作为 no-op 记录过
/// （文件驱动时期），这类老开发库升级后**不会**重跑旧迁移，导致表缺失、
/// 建卡报错；而正式发布库从未记录过该版本，旧迁移会正常建表。为让
/// 老开发库无需删库也能继续使用，这里补一条**幂等自愈**迁移：
/// - 表不存在则按最终 schema 建表（已存在则跳过）；
/// - 创建部分唯一索引前先收敛多余激活行（防老库脏数据让建索引失败）；
/// - 索引缺失则补建。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. 建表（幂等）：老开发库缺失时在此补建，schema 与 m20260817 完全一致。
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

        // 2. 收敛激活位：若老库同时存在多行 is_active=1（无索引时代留下的脏数据），
        //    部分唯一索引会创建失败。保留 card_id 升序第一张（默认卡优先）为激活，
        //    其余全部熄灭。
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "UPDATE player_profile SET is_active = 0 \
                 WHERE is_active = 1 AND card_id <> (\
                   SELECT card_id FROM player_profile \
                   WHERE is_active = 1 ORDER BY card_id LIMIT 1\
                 )",
            ))
            .await?;

        // 3. 补建部分唯一索引（保证至多一个激活人设）。
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
        // 只撤销本迁移负责的索引；表归 m20260817 所有，不在此删除。
        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "DROP INDEX IF EXISTS uq_player_profile_active",
            ))
            .await?;
        Ok(())
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
