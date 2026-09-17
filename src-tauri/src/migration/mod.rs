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
            // 「我的身份」：存档 → 身份的绑定。**必须追加在末尾**，
            // 保持时间戳单调递增，否则会打乱 sea-orm 未应用迁移的执行顺序。
            Box::new(m20260918_000001_create_save_identity::Migration),
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
