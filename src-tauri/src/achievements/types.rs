use serde::{Deserialize, Serialize};

/// 成就定义（预定义或动态注册）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementDef {
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub ach_type: String, // "common" | "rare" | "adventure"
    #[serde(default)]
    pub target_progress: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub img_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

/// 成就状态（持久化到 achievement.json）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AchievementState {
    pub unlocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocked_at: Option<String>,
    pub current_progress: u32,
}

/// 合并后的完整成就（返回给前端）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Achievement {
    pub id: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub ach_type: String,
    pub unlocked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocked_at: Option<String>,
    pub current_progress: u32,
    pub target_progress: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub img_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

impl Achievement {
    /// 合并 AchievementDef + AchievementState → Achievement
    pub fn from_parts(id: String, def: &AchievementDef, state: &AchievementState) -> Self {
        Self {
            id,
            title: def.title.clone(),
            description: def.description.clone(),
            ach_type: def.ach_type.clone(),
            unlocked: state.unlocked,
            unlocked_at: state.unlocked_at.clone(),
            current_progress: state.current_progress,
            target_progress: def.target_progress,
            img_url: def.img_url.clone(),
            audio_url: def.audio_url.clone(),
            duration: def.duration,
        }
    }
}
