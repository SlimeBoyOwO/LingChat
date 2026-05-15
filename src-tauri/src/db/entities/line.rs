use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
pub enum LineAttribute {
    #[sea_orm(string_value = "user")]
    User,
    #[sea_orm(string_value = "system")]
    System,
    #[sea_orm(string_value = "assistant")]
    Assistant,
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "line")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    #[sea_orm(column_type = "Text")]
    pub content: String,
    pub original_emotion: Option<String>,
    pub predicted_emotion: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub tts_content: Option<String>,
    #[sea_orm(column_type = "Text", nullable)]
    pub action_content: Option<String>,
    pub audio_file: Option<String>,
    pub attribute: LineAttribute,
    pub sender_role_id: Option<i32>,
    pub display_name: Option<String>,
    pub save_id: i32,
    pub parent_line_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::save::Entity",
        from = "Column::SaveId",
        to = "super::save::Column::Id"
    )]
    Save,
    #[sea_orm(
        belongs_to = "super::role::Entity",
        from = "Column::SenderRoleId",
        to = "super::role::Column::Id"
    )]
    SenderRole,
    #[sea_orm(
        belongs_to = "Entity",
        from = "Column::ParentLineId",
        to = "Column::Id"
    )]
    ParentLine,
}

impl Related<super::save::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Save.def()
    }
}

impl Related<super::role::Entity> for Entity {
    fn to() -> RelationDef {
        via_line_perception()
    }
}

fn via_line_perception() -> RelationDef {
    super::line_perception::Relation::Role.def()
}

impl ActiveModelBehavior for ActiveModel {}
