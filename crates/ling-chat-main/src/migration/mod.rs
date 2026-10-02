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
            // 建表与删表要成对留在列表里：删掉已应用过的迁移文件会让旧数据库启动即崩。
            Box::new(m20260928_000001_create_script_events::Migration),
            Box::new(m20261001_000001_drop_script_events::Migration),
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
pub mod m20260928_000001_create_script_events;
pub mod m20261001_000001_drop_script_events;
