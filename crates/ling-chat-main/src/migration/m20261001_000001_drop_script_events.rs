use sea_orm_migration::prelude::*;

/// 收掉「剧作事件流水」那张审计表（该功能已移除）。
/// 建表那个迁移必须继续留在列表里：SeaORM 发现已应用过的迁移文件不在列表里会报错并让应用启动 panic，所以用一个新迁移删表。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ScriptEvents::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 回退不重建：这张表只为审计存在，重来一次也不需要它。
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ScriptEvents {
    Table,
}
