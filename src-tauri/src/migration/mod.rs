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
            Box::new(m20260928_000001_create_script_events::Migration),
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

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, Statement};

    /// 迁移跑不通 = App 起不来，所以每次加迁移都要有一条真跑一遍的测试。
    /// 顺带验证新表真的建出来了（`script_events` 是审计表，缺了不影响创作，
    /// 但少了它事件流水会静默丢失）。
    #[tokio::test]
    async fn migrations_apply_on_an_empty_database() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("内存库连不上");
        Migrator::up(&db, None).await.expect("迁移执行失败");

        let rows = db
            .query_all(Statement::from_string(
                db.get_database_backend(),
                "SELECT name FROM sqlite_master WHERE type='table' AND name='script_events'"
                    .to_string(),
            ))
            .await
            .expect("查表失败");
        assert_eq!(rows.len(), 1, "script_events 表没建出来");
    }
}
