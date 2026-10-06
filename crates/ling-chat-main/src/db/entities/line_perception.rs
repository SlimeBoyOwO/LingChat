use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "line_perception")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub line_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub role_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::line::Entity",
        from = "Column::LineId",
        to = "super::line::Column::Id"
    )]
    Line,
    #[sea_orm(
        belongs_to = "super::role::Entity",
        from = "Column::RoleId",
        to = "super::role::Column::Id"
    )]
    Role,
}

impl Related<super::line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Line.def()
    }
}

impl Related<super::role::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Role.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
