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
            Box::new(m20261004_000001_create_role_hotword::Migration),
            Box::new(m20261005_000001_create_global_hotword::Migration),
            Box::new(m20261009_000001_create_hotword_library::Migration),
            Box::new(m20261010_000001_drop_hotword_lang::Migration),
            Box::new(m20261011_000001_remove_asr_language::Migration),
            Box::new(m20261012_000001_normalize_hotword_weight::Migration),
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
pub mod m20261004_000001_create_role_hotword;
pub mod m20261005_000001_create_global_hotword;
pub mod m20261009_000001_create_hotword_library;
pub mod m20261010_000001_drop_hotword_lang;
pub mod m20261011_000001_remove_asr_language;
pub mod m20261012_000001_normalize_hotword_weight;
