use sea_orm_migration::prelude::*;

use crate::db::entities::hotword::{KIND_GLOBAL, KIND_LIBRARY, KIND_ROLE};

/// 三张作用域热词表（`role_hotword` / `global_hotword` / `library_hotword`）合并为单表
/// `hotword`：用 `kind` 区分作用域，`role_id` / `category_id` 承载归属（issue #951）。
///
/// 热词未发版，旧表直接丢弃、不搬数据。旧迁移文件已删除，跑过 dev 版的库须先用
/// `scripts/repair-hotword-migration.mjs` 清掉账本里的旧记录，否则启动会对账失败。
///
/// sea-orm 1.x 不为 SQLite 迁移开事务，故整段可重入：先 `IF EXISTS` 清场（含上一轮失败
/// 留下的新表），再建表建索引。唯一性用部分唯一索引表达，不写 `CHECK`：权重口径由
/// `normalize_weight` 在写入前统一归一，DDL 不再重复一遍。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            LegacyHotword::RoleHotword,
            LegacyHotword::GlobalHotword,
            LegacyHotword::LibraryHotword,
        ] {
            manager
                .drop_table(Table::drop().if_exists().table(table).to_owned())
                .await?;
        }
        // 上一轮失败可能留下半成品，新表也一并清掉
        manager
            .drop_table(Table::drop().if_exists().table(Hotword::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(HotwordCategory::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(HotwordCategory::Table)
                    .col(
                        ColumnDef::new(HotwordCategory::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(HotwordCategory::Name).text().not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_hotword_category_name")
                    .table(HotwordCategory::Table)
                    .col(HotwordCategory::Name)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Hotword::Table)
                    .col(
                        ColumnDef::new(Hotword::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Hotword::Kind).text().not_null())
                    .col(ColumnDef::new(Hotword::RoleId).integer())
                    .col(ColumnDef::new(Hotword::CategoryId).integer())
                    .col(ColumnDef::new(Hotword::Text).text().not_null())
                    .col(
                        ColumnDef::new(Hotword::Weight)
                            .integer()
                            .not_null()
                            .default(4),
                    )
                    .to_owned(),
            )
            .await?;

        // SQLite 的复合唯一索引对 NULL 不生效，四种作用域各建一条部分唯一索引
        manager
            .create_index(
                Index::create()
                    .name("ux_hotword_role_text")
                    .table(Hotword::Table)
                    .col(Hotword::RoleId)
                    .col(Hotword::Text)
                    .unique()
                    .and_where(Expr::col(Hotword::Kind).eq(KIND_ROLE))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_hotword_global_text")
                    .table(Hotword::Table)
                    .col(Hotword::Text)
                    .unique()
                    .and_where(Expr::col(Hotword::Kind).eq(KIND_GLOBAL))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_hotword_library_category_text")
                    .table(Hotword::Table)
                    .col(Hotword::CategoryId)
                    .col(Hotword::Text)
                    .unique()
                    .and_where(Expr::col(Hotword::Kind).eq(KIND_LIBRARY))
                    .and_where(Expr::col(Hotword::CategoryId).is_not_null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_hotword_library_uncategorized_text")
                    .table(Hotword::Table)
                    .col(Hotword::Text)
                    .unique()
                    .and_where(Expr::col(Hotword::Kind).eq(KIND_LIBRARY))
                    .and_where(Expr::col(Hotword::CategoryId).is_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    /// 旧的三张作用域表已随迁移文件删除，回滚只还原到「没有热词表」。
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().if_exists().table(Hotword::Table).to_owned())
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(HotwordCategory::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum LegacyHotword {
    RoleHotword,
    GlobalHotword,
    LibraryHotword,
}

#[derive(DeriveIden)]
enum HotwordCategory {
    Table,
    Id,
    Name,
}

#[derive(DeriveIden)]
enum Hotword {
    Table,
    Id,
    Kind,
    RoleId,
    CategoryId,
    Text,
    Weight,
}
