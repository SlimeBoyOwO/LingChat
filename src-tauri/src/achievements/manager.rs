use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use crate::achievements::types::{Achievement, AchievementDef, AchievementState};

// ========== 默认成就定义 ==========

static DEFAULT_ACHIEVEMENTS: LazyLock<HashMap<&'static str, AchievementDef>> =
    LazyLock::new(|| {
        HashMap::from([
            (
                "first_chat",
                AchievementDef {
                    title: "初次见面".into(),
                    description: "与钦灵完成了第一次对话".into(),
                    ach_type: "common".into(),
                    target_progress: 1,
                    img_url: None,
                    audio_url: None,
                    duration: None,
                    hidden: false,
                },
            ),
            (
                "chat_master",
                AchievementDef {
                    title: "话痨".into(),
                    description: "与钦灵完成了 10 次对话".into(),
                    ach_type: "common".into(),
                    target_progress: 10,
                    img_url: None,
                    audio_url: None,
                    duration: None,
                    hidden: false,
                },
            ),
            (
                "first_pomodoro",
                AchievementDef {
                    title: "专注时刻".into(),
                    description: "第一次使用番茄钟".into(),
                    ach_type: "common".into(),
                    target_progress: 1,
                    img_url: None,
                    audio_url: None,
                    duration: None,
                    hidden: false,
                },
            ),
            (
                "night_owl",
                AchievementDef {
                    title: "夜猫子".into(),
                    description: "在深夜（23:00-04:00）与钦灵聊天".into(),
                    ach_type: "rare".into(),
                    target_progress: 1,
                    img_url: None,
                    audio_url: None,
                    duration: None,
                    hidden: false,
                },
            ),
            (
                "see_through",
                AchievementDef {
                    title: "看穿一切".into(),
                    description: "背景一透明，感觉整个人都被你看透了。……算了，就允许你看一会儿吧。\n（实验性透明，有bug也是很正常的）".into(),
                    ach_type: "rare".into(),
                    target_progress: 1,
                    img_url: None,
                    audio_url: None,
                    duration: Some(6000),
                    hidden: true,
                },
            ),
        ])
    });

// ========== AchievementManager ==========

/// 内置成就的键名集合。剧本编辑器的校验器用它检查「解锁成就」事件是否与
/// 内置成就重名（重名会覆盖内置定义，应当禁止）。
pub fn builtin_achievement_ids() -> Vec<&'static str> {
    DEFAULT_ACHIEVEMENTS.keys().copied().collect()
}

pub struct AchievementManager {
    file_path: PathBuf,
    /// 动态注册的成就（如冒险脚本定义的 completion_achievements）
    dynamic_achievements: HashMap<String, AchievementDef>,
    /// 运行时状态（解锁、进度等）
    state: HashMap<String, AchievementState>,
    dirty: bool,
}

impl AchievementManager {
    pub fn new(data_dir: &Path) -> Self {
        let file_path = data_dir.join("game_data").join("achievement.json");
        let state = Self::load_from_file(&file_path);
        let mut manager = Self {
            file_path,
            dynamic_achievements: HashMap::new(),
            state,
            dirty: false,
        };
        // 确保所有默认成就都有状态条目
        manager.ensure_default_states();
        // 检查是否有新成就需要初始化
        let needs_save = DEFAULT_ACHIEVEMENTS
            .keys()
            .any(|id| !manager.state.contains_key(*id));
        if needs_save {
            manager.save();
        } else {
            manager.dirty = false;
        }
        manager
    }

    fn load_from_file(file_path: &Path) -> HashMap<String, AchievementState> {
        if !file_path.exists() {
            return HashMap::new();
        }
        std::fs::read_to_string(file_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn ensure_default_states(&mut self) {
        for id in DEFAULT_ACHIEVEMENTS.keys() {
            self.state
                .entry(id.to_string())
                .or_insert_with(|| AchievementState {
                    unlocked: false,
                    unlocked_at: None,
                    current_progress: 0,
                });
        }
    }

    // ========== 公共 API ==========

    pub fn get_all_achievements(&self) -> HashMap<String, Achievement> {
        let mut result = HashMap::new();
        for (id, def) in DEFAULT_ACHIEVEMENTS.iter() {
            let state = self
                .state
                .get(*id)
                .cloned()
                .unwrap_or_else(|| AchievementState {
                    unlocked: false,
                    unlocked_at: None,
                    current_progress: 0,
                });
            let mut ach = Achievement::from_parts(id.to_string(), def, &state);
            Self::mask_hidden(&mut ach, def, &state);
            result.insert(id.to_string(), ach);
        }
        for (id, def) in &self.dynamic_achievements {
            let state = self
                .state
                .get(id)
                .cloned()
                .unwrap_or_else(|| AchievementState {
                    unlocked: false,
                    unlocked_at: None,
                    current_progress: 0,
                });
            let mut ach = Achievement::from_parts(id.clone(), def, &state);
            Self::mask_hidden(&mut ach, def, &state);
            result.insert(id.clone(), ach);
        }
        result
    }

    /// 隐藏成就未解锁时，对其标题/描述脱敏
    fn mask_hidden(ach: &mut Achievement, def: &AchievementDef, state: &AchievementState) {
        if def.hidden && !state.unlocked {
            ach.title = "???".to_string();
            ach.description = "达成特定条件后解锁".to_string();
        }
    }

    /// 注册动态成就（如冒险的 completion_achievements）
    pub fn register_achievement(&mut self, id: String, def: AchievementDef) {
        self.dynamic_achievements.insert(id.clone(), def);
        if !self.state.contains_key(&id) {
            self.state.insert(
                id,
                AchievementState {
                    unlocked: false,
                    unlocked_at: None,
                    current_progress: 0,
                },
            );
        }
    }

    /// 增加成就进度，到达目标时自动解锁
    pub fn increment_progress(&mut self, id: &str, amount: u32) -> Option<Achievement> {
        let def = self.get_def(id)?;

        let state = self
            .state
            .entry(id.to_string())
            .or_insert_with(|| AchievementState {
                unlocked: false,
                unlocked_at: None,
                current_progress: 0,
            });

        if state.unlocked {
            return None;
        }

        state.current_progress = state.current_progress.saturating_add(amount);
        let target = def.target_progress.max(1);

        if state.current_progress >= target {
            state.current_progress = target;
            drop(def); // release immutable borrow before unlock()
            self.unlock(id)
        } else {
            self.dirty = true;
            None
        }
    }

    /// 检查成就是否已解锁
    pub fn is_unlocked(&self, id: &str) -> bool {
        self.state.get(id).map(|s| s.unlocked).unwrap_or(false)
    }

    /// 直接解锁成就
    pub fn unlock(&mut self, id: &str) -> Option<Achievement> {
        let def = self.get_def(id)?;
        let state = self
            .state
            .entry(id.to_string())
            .or_insert_with(|| AchievementState {
                unlocked: false,
                unlocked_at: None,
                current_progress: 0,
            });

        if state.unlocked {
            return None;
        }

        state.unlocked = true;
        state.unlocked_at = Some(chrono::Utc::now().to_rfc3339());
        state.current_progress = def.target_progress.max(1);

        let achievement = Achievement::from_parts(id.to_string(), &def, state);
        // 解锁是重要事件，立即保存
        self.save();
        Some(achievement)
    }

    // ========== 内部方法 ==========

    fn get_def(&self, id: &str) -> Option<AchievementDef> {
        // 先查默认，再查动态
        DEFAULT_ACHIEVEMENTS
            .get(id)
            .cloned()
            .or_else(|| self.dynamic_achievements.get(id).cloned())
    }

    pub fn save(&mut self) {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(&self.state) {
            let _ = std::fs::write(&self.file_path, json);
        }
        self.dirty = false;
    }
}
