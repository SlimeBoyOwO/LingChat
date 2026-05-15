use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{DateTime, Local};
use sea_orm::DatabaseConnection;
use serde_json::Value;

use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::types::{GameLine, GameRole, LineBase, Player, ScriptStatus};
use crate::db::entities::line::LineAttribute;

/// 存储所有运行时共享的游戏状态。
pub struct GameStatus {
    pub player: Player,

    /// 当前加载的场景描述（若为 None 则处于普通自由对话模式）
    pub scene_description: Option<String>,
    /// 台词列表，用于记忆构建和历史记忆
    pub line_list: Vec<GameLine>,

    pub role_manager: GameRoleManager,
    /// 当前对话角色的 role_id；作为 LLM 传输入的对象，使用本角色的记忆
    pub current_role_id: Option<i32>,
    /// 舞台角色 role_id 列表：用于展示舞台上角色的信息（保持顺序）
    pub onstage_role_ids: Vec<i32>,
    /// 在场角色 role_id 集合：只有在场的角色才能感知到台词
    pub present_role_ids: HashSet<i32>,
    /// 游戏主角的 role_id（剧本模式冒险的主角）
    pub main_role_id: Option<i32>,

    pub current_scene: String,
    pub background: String,
    pub present_pic: String,
    pub background_music: String,
    pub background_effect: String,

    pub global_variables: HashMap<String, Value>,
    pub completed_scripts: HashSet<String>,
    pub last_dialog_time: Option<DateTime<Local>>,

    pub script_status: Option<ScriptStatus>,

    /// 当前激活的存档 ID（用于 MemoryBank 持久化/载入/自动压缩）
    pub active_save_id: Option<i32>,
}

impl GameStatus {
    pub fn new(role_manager: GameRoleManager) -> Self {
        Self {
            player: Player::default(),
            scene_description: None,
            line_list: Vec::new(),
            role_manager,
            current_role_id: None,
            onstage_role_ids: Vec::new(),
            present_role_ids: HashSet::new(),
            main_role_id: None,
            current_scene: String::new(),
            background: String::new(),
            present_pic: String::new(),
            background_music: String::new(),
            background_effect: String::new(),
            global_variables: HashMap::new(),
            completed_scripts: HashSet::new(),
            last_dialog_time: None,
            script_status: None,
            active_save_id: None,
        }
    }

    pub async fn get_role<'a>(
        &'a mut self,
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<&'a mut GameRole> {
        self.role_manager.get_role(db, role_id).await
    }

    /// 追加台词，记录当前在场者为感知列表，并刷新相关角色的记忆。
    pub async fn add_line(&mut self, db: &DatabaseConnection, line: LineBase) -> Result<()> {
        let perceived: Vec<i32> = self.present_role_ids.iter().copied().collect();
        let game_line = GameLine::from_base(line, perceived);
        self.line_list.push(game_line);
        self.refresh_memories(db).await?;
        Ok(())
    }

    pub async fn refresh_memories(&mut self, db: &DatabaseConnection) -> Result<()> {
        self.role_manager
            .sync_memories(db, &self.line_list, None)
            .await
    }

    // ============ 全局变量便捷方法 ============

    pub fn set_variable(&mut self, key: impl Into<String>, value: Value) {
        self.global_variables.insert(key.into(), value);
    }

    pub fn get_variable(&self, key: &str) -> Option<&Value> {
        self.global_variables.get(key)
    }

    /// 非系统消息数量（用于羁绊冒险解锁条件检测）
    pub fn chat_message_count(&self) -> usize {
        self.line_list
            .iter()
            .filter(|l| !matches!(l.attribute(), LineAttribute::System))
            .count()
    }

    // ============ 舞台管理 ============

    pub fn onstage_role(&mut self, role_id: i32) {
        if !self.onstage_role_ids.contains(&role_id) {
            self.onstage_role_ids.push(role_id);
        }
        self.present_role_ids.insert(role_id);
    }

    pub fn offstage_role(&mut self, role_id: i32) {
        self.onstage_role_ids.retain(|id| *id != role_id);
        self.present_role_ids.remove(&role_id);
    }

    pub fn reactivate_all_voice_makers(&self) {
        self.role_manager.reactivate_all_voice_makers();
    }
}
