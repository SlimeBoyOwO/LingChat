use std::collections::{BTreeSet, HashMap};

use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set, TransactionTrait,
};
use sea_orm_migration::prelude::*;

use crate::db::entities::line as line_entity;
use crate::db::entities::role as role_entity;
use crate::db::entities::role::RoleType;
use crate::db::entities::role_relation as role_relation_entity;
use crate::db::entities::save as save_entity;

/// 玩家身份并入 role：身份表、存档绑定、认知关系与台词归属一次迁移。
///
/// 身份 id 取四张旧表的并集，不假设 `player_identity` 与存档/关系/台词同步一致：
/// 索引表落后于存档或台词时，照样给这些 id 建玩家角色行。
///
/// 不可回滚：旧表与身份 id 无法还原。
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // role：身份卡稳定键 + 身份 YAML 关系只导入一次的标记
        manager
            .alter_table(
                Table::alter()
                    .table(Role::Table)
                    .add_column(ColumnDef::new(Role::IdentityId).text().null())
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Role::Table)
                    .add_column(
                        ColumnDef::new(Role::IdentityRelationsSeeded)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        // SQLite 的 UNIQUE 允许多个 NULL，等价于只对非空 identity_id 建唯一索引
        manager
            .create_index(
                Index::create()
                    .name("uq_role_identity")
                    .table(Role::Table)
                    .col(Role::IdentityId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 认知关系：role → role 的多对多
        manager
            .create_table(
                Table::create()
                    .table(RoleRelation::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(RoleRelation::RoleId).integer().not_null())
                    .col(
                        ColumnDef::new(RoleRelation::TargetRoleId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RoleRelation::Text)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(ColumnDef::new(RoleRelation::InitialAffection).integer())
                    .col(ColumnDef::new(RoleRelation::UpdatedAt).text().not_null())
                    .primary_key(
                        Index::create()
                            .name("pk_role_relation")
                            .col(RoleRelation::RoleId)
                            .col(RoleRelation::TargetRoleId)
                            .primary(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RoleRelation::Table, RoleRelation::RoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(RoleRelation::Table, RoleRelation::TargetRoleId)
                            .to(Role::Table, Role::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ix_role_relation_target")
                    .table(RoleRelation::Table)
                    .col(RoleRelation::TargetRoleId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 存档绑定玩家角色：应用层引用，不建外键（避免重建 save 表）
        manager
            .alter_table(
                Table::alter()
                    .table(Save::Table)
                    .add_column(ColumnDef::new(Save::PlayerRoleId).integer().null())
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ix_save_player_role")
                    .table(Save::Table)
                    .col(Save::PlayerRoleId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // 数据搬运在事务里做：中途失败不留半套玩家角色与绑定
        let txn = manager.get_connection().begin().await?;
        match backfill(&txn).await {
            Ok(()) => txn.commit().await?,
            Err(e) => {
                let _ = txn.rollback().await;
                return Err(e);
            },
        }

        // 台词不再单独记录身份，归属靠 sender_role_id
        manager
            .alter_table(
                Table::alter()
                    .table(Line::Table)
                    .drop_column(Line::IdentityId)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .table(SaveIdentity::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(RoleIdentityRelation::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(PlayerIdentity::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // 旧表已删除，身份 id 与关系行无法还原。
        Err(DbErr::Migration(
            "m20261114_000001 不可回滚：身份表已并入 role".into(),
        ))
    }
}

/// 建玩家角色行、搬关系、回填存档绑定、改台词归属。
async fn backfill<C: ConnectionTrait>(conn: &C) -> Result<(), DbErr> {
    // 旧表里出现过的身份 id：索引用名字，其余表只保证 id 存在
    let mut names: HashMap<String, String> = legacy::player_identity::Entity::find()
        .all(conn)
        .await?
        .into_iter()
        .map(|row| (row.identity_id, row.name))
        .collect();
    let mut identity_ids: BTreeSet<String> = names.keys().cloned().collect();
    for row in legacy::save_identity::Entity::find().all(conn).await? {
        identity_ids.insert(row.identity_id);
    }
    for row in legacy::role_identity_relation::Entity::find()
        .all(conn)
        .await?
    {
        identity_ids.insert(row.identity_id);
    }
    for row in legacy::line::Entity::find()
        .filter(legacy::line::Column::IdentityId.is_not_null())
        .all(conn)
        .await?
    {
        if let Some(identity_id) = row.identity_id {
            identity_ids.insert(identity_id);
        }
    }
    identity_ids.retain(|id| !id.trim().is_empty());

    let mut identity_to_role: HashMap<String, i32> = role_entity::Entity::find()
        .filter(role_entity::Column::IdentityId.is_not_null())
        .all(conn)
        .await?
        .into_iter()
        .filter_map(|row| row.identity_id.map(|id| (id, row.id)))
        .collect();

    // 只有 id 没有名字的身份先以 id 作名字，启动后的 sync_player_roles 会用卡文件补正
    for identity_id in identity_ids {
        if identity_to_role.contains_key(&identity_id) {
            continue;
        }
        let name = names
            .remove(&identity_id)
            .unwrap_or_else(|| identity_id.clone());
        let inserted = role_entity::ActiveModel {
            name: Set(name),
            role_type: Set(RoleType::Player),
            resource_folder: Set(None),
            identity_id: Set(Some(identity_id.clone())),
            ..Default::default()
        }
        .insert(conn)
        .await?;
        identity_to_role.insert(identity_id, inserted.id);
    }

    // 旧认知关系换成玩家角色 id
    for row in legacy::role_identity_relation::Entity::find()
        .all(conn)
        .await?
    {
        let Some(target_role_id) = identity_to_role.get(&row.identity_id) else {
            continue;
        };
        role_relation_entity::ActiveModel {
            role_id: Set(row.role_id),
            target_role_id: Set(*target_role_id),
            text: Set(row.text),
            initial_affection: Set(row.initial_affection),
            updated_at: Set(row.updated_at),
        }
        .insert(conn)
        .await?;
    }

    for row in legacy::save_identity::Entity::find().all(conn).await? {
        let Some(role_id) = identity_to_role.get(&row.identity_id) else {
            continue;
        };
        save_entity::Entity::update_many()
            .col_expr(save_entity::Column::PlayerRoleId, Expr::value(*role_id))
            .filter(save_entity::Column::Id.eq(row.save_id))
            .exec(conn)
            .await?;
    }

    // 历史玩家台词改指向对应玩家角色；无身份归属的 0 保留
    let mut lines_by_role: HashMap<i32, Vec<i32>> = HashMap::new();
    for row in legacy::line::Entity::find()
        .filter(legacy::line::Column::IdentityId.is_not_null())
        .filter(legacy::line::Column::Attribute.eq("user"))
        .all(conn)
        .await?
    {
        let Some(identity_id) = row.identity_id else {
            continue;
        };
        let Some(role_id) = identity_to_role.get(&identity_id) else {
            continue;
        };
        lines_by_role.entry(*role_id).or_default().push(row.id);
    }
    for (role_id, line_ids) in lines_by_role {
        line_entity::Entity::update_many()
            .col_expr(
                line_entity::Column::SenderRoleId,
                Expr::value(Some(role_id)),
            )
            .filter(line_entity::Column::Id.is_in(line_ids))
            .exec(conn)
            .await?;
    }

    Ok(())
}

/// 迁移期专用的旧表模型：迁移结束时这些表会被删除，不进 db/entities。
mod legacy {
    pub mod player_identity {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "player_identity")]
        pub struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub identity_id: String,
            pub name: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    pub mod save_identity {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "save_identity")]
        pub struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub save_id: i32,
            pub identity_id: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    pub mod role_identity_relation {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "role_identity_relation")]
        pub struct Model {
            #[sea_orm(primary_key, auto_increment = false)]
            pub role_id: i32,
            #[sea_orm(primary_key, auto_increment = false)]
            pub identity_id: String,
            pub text: String,
            pub initial_affection: Option<i32>,
            pub updated_at: String,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }

    /// 只读 identity_id 列；迁移末尾会删掉这一列，所以不能复用现行 line 实体。
    pub mod line {
        use sea_orm::entity::prelude::*;

        #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
        #[sea_orm(table_name = "line")]
        pub struct Model {
            #[sea_orm(primary_key)]
            pub id: i32,
            pub attribute: String,
            pub identity_id: Option<String>,
        }

        #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
        pub enum Relation {}

        impl ActiveModelBehavior for ActiveModel {}
    }
}

#[derive(DeriveIden)]
enum Role {
    Table,
    Id,
    IdentityId,
    IdentityRelationsSeeded,
}

#[derive(DeriveIden)]
enum Save {
    Table,
    PlayerRoleId,
}

#[derive(DeriveIden)]
enum Line {
    Table,
    IdentityId,
}

#[derive(DeriveIden)]
enum RoleRelation {
    Table,
    RoleId,
    TargetRoleId,
    Text,
    InitialAffection,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum SaveIdentity {
    Table,
}

#[derive(DeriveIden)]
enum RoleIdentityRelation {
    Table,
}

#[derive(DeriveIden)]
enum PlayerIdentity {
    Table,
}
