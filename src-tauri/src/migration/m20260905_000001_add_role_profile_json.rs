use sea_orm::Statement;
use sea_orm_migration::prelude::*;

/// 为 role 增加 profile_json 列（统一实体后的实体人设扩展）。
///
/// 稳定性说明：只加可空列，SQLite 元数据级操作，不动已有行/索引；
/// 旧库升级后旧行该列自动为 NULL，读取侧按 NULL=默认人设处理（玩家实体首次写入前也是 NULL）。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 防御性预检：SQLite 的 ADD COLUMN 没有 IF NOT EXISTS 语法，先查
        // pragma_table_info 确认列不存在再执行，防 seaql_migrations 记录
        // 异常（如被手动清空）导致重复执行时报「duplicate column」。
        let rows = manager
            .get_connection()
            .query_all(Statement::from_string(
                manager.get_database_backend(),
                "SELECT name FROM pragma_table_info('role') WHERE name = 'profile_json'"
                    .to_string(),
            ))
            .await?;
        if rows.is_empty() {
            manager
                .alter_table(
                    Table::alter()
                        .table(Role::Table)
                        .add_column(ColumnDef::new(Role::ProfileJson).text().null())
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Role::Table)
                    .drop_column(Role::ProfileJson)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Role {
    Table,
    ProfileJson,
}
