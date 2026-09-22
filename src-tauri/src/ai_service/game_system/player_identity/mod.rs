//! 「我的身份」：可切换的玩家身份卡（名字 / 副标题 / 提示词 / 关系）。
//!
//! 刻意做薄：玩家发言在库里仍是角色 0，身份卡只是「玩家对象」的填充来源，
//! 所以消息表 / 角色表 / 记忆系统一处都不用改。多个可操作角色的扩展点见
//! [`resolve`]（不可变身份 id、关系键带命名空间、关系按 (说话者, 目标) 解析）。
//!
//! 存储：`<data_dir>/game_data/my_identities/`（`<文件夹>/identity.yml` + `_current.json`）。
//! 旧版本不认识这个目录，天然向前兼容。

pub mod guard;
pub mod persona;
pub mod resolve;
pub mod role_relations;
pub mod store;

pub use guard::{ensure_identity_mutable, ensure_identity_switchable};
pub use resolve::{
    RelationEndpoint, ScenePeer, build_peers_block, build_player_block, resolve_relation,
};
pub use store::IdentityStore;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// 身份卡。`name` / `subtitle` / `prompt` 与玩家对象对齐，载入时直接填充。
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlayerIdentity {
    /// 不可变 id；关系键与存档记录都用它，改文件夹名不会断链。
    #[serde(default)]
    pub id: String,
    /// 我的名字（注入 prompt 时作为「用户名称 / 玩家名」）
    #[serde(default)]
    pub name: String,
    /// 副标题（可选）
    #[serde(default)]
    pub subtitle: String,
    /// 身份提示词：注入聊天上下文，作为「我」的身份设定
    #[serde(default)]
    pub prompt: String,
    /// 头像文件名（仅 UI 展示用，不进上下文；本轮不提供图片读取命令）
    #[serde(default)]
    pub avatar: Option<String>,
    /// 我对各个对象的关系。键为 `ai:<角色文件夹>` 或 `me:<身份 id>`。
    #[serde(default)]
    pub relations: HashMap<String, String>,
    /// 运行时标记：这张卡是按老规则临时合成、且尚未落盘成功的。不写入文件。
    #[serde(skip)]
    pub synthetic: bool,
}

impl PlayerIdentity {
    /// 名字为空时的兜底展示名（与老代码的 `user_name` 默认值保持一致）。
    pub fn display_name(&self) -> &str {
        if self.name.trim().is_empty() {
            "用户"
        } else {
            self.name.trim()
        }
    }
}

/// 身份列表项（给前端卡片列表用，避免把全部关系都发过去）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PlayerIdentitySummary {
    pub id: String,
    pub name: String,
    pub subtitle: String,
    pub prompt: String,
    pub has_avatar: bool,
    pub relation_count: usize,
    /// 是否是当前正在使用的身份
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
