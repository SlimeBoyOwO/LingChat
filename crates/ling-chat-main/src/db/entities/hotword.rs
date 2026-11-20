//! 热词表：角色 / 全局 / 词库三种作用域共用一张表，`kind` 区分。
//!
//! 归属列可空：`kind = role` 用 `role_id`，`kind = library` 用 `category_id`（NULL = 未分类），
//! 全局两列恒为空。不写外键：删角色时连接已关外键，LAN 镜像又在事务内，级联只能落在应用层。
//!
//! `weight` 用 `i32` 存越界存量（`u8` 读回会报错），取值口径见 [`normalize_weight`]。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub const KIND_ROLE: &str = "role";
pub const KIND_GLOBAL: &str = "global";
pub const KIND_LIBRARY: &str = "library";

pub const MIN_WEIGHT: i32 = 1;
pub const WEIGHT_CAP: i32 = 5;
/// 超级热词的特殊值（百炼口径：`[1, 5]` 为普通热词，`50` 为超级热词）。
pub const SUPER_WEIGHT: i32 = 50;
/// 一次请求内超级热词的条数上限，超出由发送层降级。
pub const SUPER_COUNT_LIMIT: usize = 50;

/// 把任意 `weight` 归一到 `[1, 5] ∪ {50}`：非正数抬到 1，`6..=49` 降为 5，`>= 50` 归为 50。
///
/// 放在实体层是因为 `lan_sync/db_sync.rs` 的镜像导入要在写入前归一远端 payload 的脏值，
/// 而它只依赖实体；口径须与 `ai_service/asr/provider_types.rs` 的 `Hotword` 一致。
pub fn normalize_weight(weight: i32) -> i32 {
    if weight <= 0 {
        MIN_WEIGHT
    } else if weight <= WEIGHT_CAP {
        weight
    } else if weight < SUPER_WEIGHT {
        WEIGHT_CAP
    } else {
        SUPER_WEIGHT
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "hotword")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub kind: String,
    /// `kind = role` 时非空。
    pub role_id: Option<i32>,
    /// `kind = library` 时可为分类 id，NULL = 未分类。
    pub category_id: Option<i32>,
    #[sea_orm(column_type = "Text")]
    pub text: String,
    pub weight: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
