use sea_orm_migration::prelude::*;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20240101_000001_create_tables::Migration),
            Box::new(m20260727_000002_add_line_tool_call::Migration),
            Box::new(m20260729_000002_add_line_thinking::Migration),
        ]
    }
}

pub mod m20240101_000001_create_tables;
pub mod m20260727_000002_add_line_tool_call;
pub mod m20260729_000002_add_line_thinking;
