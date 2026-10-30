use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_tables::Migration),
            Box::new(m20260727_000002_add_line_tool_call::Migration),
            Box::new(m20260729_000002_add_line_thinking::Migration),
            Box::new(m20260803_000001_create_skill_agent_tables::Migration),
            Box::new(m20260807_000001_add_skill_agent_reasoning::Migration),
            Box::new(m20260814_000001_add_skill_agent_token_usage::Migration),
            Box::new(m20260815_000001_add_skill_agent_cached_tokens::Migration),
            // 「我的身份」：存档 → 身份的绑定。必须追加在末尾，保持时间戳单调递增。
            Box::new(m20260918_000001_create_save_identity::Migration),
            // 台词行归属身份：只加列。必须追加在末尾，保持时间戳单调递增。
            Box::new(m20261015_000001_add_line_identity_id::Migration),
            // 身份索引表（文件为真源，本表供查询）：必须追加在末尾，保持时间戳单调递增。
            Box::new(m20261016_000001_create_player_identity_index::Migration),
            // 工具轮标识：前导台词、载体与工具结果共享，记忆构建器据此分组。
            Box::new(m20261020_000001_add_line_round_id::Migration),
            // 角色与玩家身份的认知关系（多对多），从 relations.yml 迁入。
            Box::new(m20261030_000001_create_role_identity_relation::Migration),
            // 该表追加初始好感度覆写列。
            Box::new(m20261030_000002_add_role_identity_affection::Migration),
        ]
    }
}

pub mod m20240101_000001_create_tables;
pub mod m20260727_000002_add_line_tool_call;
pub mod m20260729_000002_add_line_thinking;
pub mod m20260803_000001_create_skill_agent_tables;
pub mod m20260807_000001_add_skill_agent_reasoning;
pub mod m20260814_000001_add_skill_agent_token_usage;
pub mod m20260815_000001_add_skill_agent_cached_tokens;
pub mod m20260918_000001_create_save_identity;
pub mod m20261015_000001_add_line_identity_id;
pub mod m20261016_000001_create_player_identity_index;
pub mod m20261020_000001_add_line_round_id;
pub mod m20261030_000001_create_role_identity_relation;
pub mod m20261030_000002_add_role_identity_affection;
