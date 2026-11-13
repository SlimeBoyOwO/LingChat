//! 「我的身份」：可切换的玩家身份卡（名字 / 副标题 / 提示词 / 关系）。

pub mod bootstrap;
pub mod guard;
pub mod migration;
pub mod persona;
pub mod resolve;
pub mod role_relations;
pub mod runtime_relations;
pub mod store;

pub use guard::{RoleRelationStatus, ensure_identity_switchable, relation_ready_all};
pub use migration::MigrationReport;
pub use resolve::{
    AliasEntry, RelationEndpoint, ScenePeer, build_peers_block, build_player_block,
    effective_user_alias, resolve_relation,
};
pub use store::{IdentityStore, PrimaryIdentityMarker};

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerIdentity {
    /// 不可变 id；关系键与存档记录都用它。
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub subtitle: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub avatar: Option<String>,
    /// 我对各个对象的出厂关系，键为 `ai:<角色文件夹>` 或 `me:<身份 id>`。
    #[serde(default)]
    pub relations: HashMap<String, String>,
    /// 该身份绑定的角色文件夹（旧角色卡迁移生成时写入），角色卡据此反查自己的身份。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_folders: Vec<String>,
    /// 本数据目录唯一的主玩家身份；老数据迁移后只有它为 true。
    #[serde(default)]
    pub is_primary: bool,
}

impl PlayerIdentity {
    pub fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            "用户"
        } else {
            self.name.trim()
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PlayerIdentitySummary {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub prompt: String,
    pub has_avatar: bool,
    pub relation_count: usize,
    pub is_current: bool,
}

impl PlayerIdentitySummary {
    pub fn from_identity(identity: &PlayerIdentity, current_id: Option<&str>) -> Self {
        Self {
            id: identity.id.clone(),
            name: identity.display_name().to_string(),
            subtitle: identity.subtitle.clone(),
            prompt: identity.prompt.clone(),
            has_avatar: identity
                .avatar
                .as_deref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false),
            relation_count: identity
                .relations
                .values()
                .filter(|v| !v.trim().is_empty())
                .count(),
            is_current: current_id.map(|c| c == identity.id).unwrap_or(false),
        }
    }
}
