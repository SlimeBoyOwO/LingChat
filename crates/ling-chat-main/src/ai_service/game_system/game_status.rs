use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Local};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::game_system::status_ops::{Pending, PromptLine, RequestScoped};
use crate::ai_service::types::{GameLine, GameRole, LineBase, Player, ScriptStatus};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::{PromptRole, REPLY_LENGTH_VAR_KEY, ReplyLengthMode};

/// 全局变量键：剧本临时背景（值 = 解析后的背景图绝对路径）。
const BACKGROUND_OVERRIDE_VAR_KEY: &str = "background.override";

/// 存储所有运行时共享的游戏状态。
pub struct GameStatus {
    pub player: Player,

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

    pub background: String,
    pub present_pic: String,
    pub background_music: String,
    pub background_effect: String,

    /// 当前用户选择的场景 ID（对应 scenes.json 中的场景）
    pub current_scene_id: Option<String>,
    /// 上一次 process_message 处理时的场景 ID，用于检测场景切换
    pub last_processed_scene_id: Option<String>,

    pub global_variables: HashMap<String, Value>,
    pub completed_scripts: HashSet<String>,
    pub last_dialog_time: Option<DateTime<Local>>,

    pub script_status: Option<ScriptStatus>,

    /// 当前激活的存档 ID（用于 MemoryBank 持久化/载入/自动压缩）
    pub active_save_id: Option<i32>,

    /// 试玩会话代号。
    pub preview_generation: u64,

    /// 本会话是否已触发过入场问候（内存标记，重启/清档重置）。
    /// `notify_player_entry` 靠它去重，避免重复生成问候台词；与"玩家是否在游戏里"无关。
    pub entry_greeting_done: bool,

    /// 好感度评估游标：上次评估时「真实对话」的段数。
    /// 每累计 `GodAgentConfig.affection_eval_interval` 段新对话触发一次上帝 Agent 评估。
    pub affection_eval_cursor: usize,

    /// 场景感知开关（关闭后切换场景不再触发旁白）
    pub scene_awareness_enabled: bool,

    /// 等下一轮落位的暂存：插件旁白与近期记忆（见 `status_ops`）。
    pub pending: Pending,

    /// 下一次请求取走的暂存：临时记忆与工具推荐名字（见 `status_ops`）。
    pub request: RequestScoped,
}

impl GameStatus {
    pub fn new(role_manager: GameRoleManager) -> Self {
        Self {
            player: Player::default(),
            line_list: Vec::new(),
            role_manager,
            current_role_id: None,
            onstage_role_ids: Vec::new(),
            present_role_ids: HashSet::new(),
            main_role_id: None,
            background: String::new(),
            present_pic: String::new(),
            background_music: String::new(),
            background_effect: String::new(),
            current_scene_id: None,
            last_processed_scene_id: None,
            global_variables: HashMap::new(),
            completed_scripts: HashSet::new(),
            last_dialog_time: None,
            script_status: None,
            active_save_id: None,
            preview_generation: 0,
            entry_greeting_done: false,
            affection_eval_cursor: 0,
            scene_awareness_enabled: true,
            pending: Pending::default(),
            request: RequestScoped::default(),
        }
    }

    pub async fn get_role<'a>(
        &'a mut self,
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<&'a mut GameRole> {
        let var = crate::ai_service::affection::resolve_var(
            &self.global_variables,
            role_id,
            self.player.identity_id.as_deref(),
        )
        .cloned();
        let role = self.role_manager.get_role(db, role_id).await?;
        if let Some(state) = var
            .as_ref()
            .and_then(crate::ai_service::affection::state_from_value)
        {
            role.affection = state.vector;
            role.negative = state.negative;
        }
        Ok(role)
    }

    /// 已绑档时把内存台词表同步落库一次；失败只告警。
    pub async fn persist_lines_if_bound(&self, db: &DatabaseConnection) {
        let Some(save_id) = self.active_save_id else {
            return;
        };
        if let Err(e) = crate::db::managers::save_repo::SaveRepo::sync_lines(
            db,
            save_id,
            &self.line_list,
            false,
        )
        .await
        {
            tracing::warn!("同步存档失败: {e}");
        }
    }

    /// 追加台词，记录当前在场者为感知列表，并刷新相关角色的记忆。
    pub async fn add_line(&mut self, db: &DatabaseConnection, mut line: LineBase) -> Result<()> {
        // 玩家台词行自动归属当前身份；旁白/系统行不归属（换身份后旧行保留原名）。
        if matches!(line.attribute.inner(), LineAttribute::User)
            && line.sender_role_id == Some(0)
            && line.identity_id.is_none()
            && !matches!(line.display_name.as_deref(), Some("旁白") | Some("系统"))
        {
            line.identity_id = self.player.identity_id.clone();
        }
        let perceived: Vec<i32> = self.present_role_ids.iter().copied().collect();
        self.add_line_perceived(db, line, perceived).await
    }

    pub async fn add_line_perceived(
        &mut self,
        db: &DatabaseConnection,
        line: LineBase,
        perceived_role_ids: Vec<i32>,
    ) -> Result<()> {
        let game_line = GameLine::from_base(line, perceived_role_ids);
        self.line_list.push(game_line);
        self.refresh_memories(db).await?;
        Ok(())
    }

    pub async fn refresh_memories(&mut self, db: &DatabaseConnection) -> Result<()> {
        self.role_manager
            .sync_memories(db, &self.line_list, None, false, &self.player)
            .await
    }

    /// 当前流式回复与工具回填均已结束后刷新记忆。
    /// 此时允许永久记忆把一个超长、但工具配对完整的回合整体写入摘要。
    pub async fn refresh_memories_after_completed_turn(
        &mut self,
        db: &DatabaseConnection,
    ) -> Result<()> {
        self.role_manager
            .sync_memories(db, &self.line_list, None, true, &self.player)
            .await
    }

    // ── 台词历史（上下文源）的区间查看 / 编辑，下标 1 开始，左开右闭 ──
    pub fn read_context_lines(&self, start: usize, end: usize) -> Vec<GameLine> {
        match Self::clamp_line_range(self.line_list.len(), start, end) {
            Some((lo, hi)) => self.line_list[lo..hi].to_vec(),
            None => Vec::new(),
        }
    }

    /// 用 `replacement` 替换第 `start`~`end` 条台词（含两端，1 起），并让改动真正
    /// 落进上下文。`replacement` 条数不限：空 = 删除该区间，多条 = 展开。
    pub async fn edit_context_lines(
        &mut self,
        db: &DatabaseConnection,
        generation_lock: &Arc<tokio::sync::Mutex<()>>,
        start: usize,
        end: usize,
        mut replacement: Vec<GameLine>,
    ) -> Result<usize> {
        let Some((lo, hi)) = Self::clamp_line_range(self.line_list.len(), start, end) else {
            return Err(anyhow::anyhow!(
                "编辑区间无效：start={start}, end={end}，当前共 {} 条台词",
                self.line_list.len()
            ));
        };

        // 生成中拒绝：此刻改了 line_list，会让流式任务记下的 line_index / seq 漂移。
        let _generation = generation_lock
            .try_lock()
            .map_err(|_| anyhow::anyhow!("正在生成回复，暂不能编辑历史，请稍后再试"))?;

        // System 人设行允许被编辑/删除：去掉后该角色的上下文将失去 system 前缀
        // （`sync_memories` 会记录「人设丢失」警告）；之后重新加载角色时，`game.rs`
        // 会按 `already_has_system` 判定并重新注入。
        let dropped_system = self.line_list[lo..hi]
            .iter()
            .any(|l| matches!(l.attribute(), LineAttribute::System))
            && !replacement
                .iter()
                .any(|l| matches!(l.attribute(), LineAttribute::System));
        if dropped_system {
            tracing::warn!("edit_context_lines 移除了 System 人设行（区间 {start}~{end}）");
        }

        // 未带 id 的替换行按位置继承被替换行的 id，保住存档链锚点。
        for (i, line) in replacement.iter_mut().enumerate() {
            if line.base.id.is_none() && i < hi - lo {
                line.base.id = self.line_list[lo + i].base.id;
            }
        }

        self.role_manager.invalidate_memory_history();
        let removed = self.line_list.splice(lo..hi, replacement).count();
        // 指针在编辑点之后的角色回拨到编辑起点，让这段重新进入窗口。
        self.role_manager.rewind_memory_pointers(lo).await;
        self.refresh_memories(db).await?;

        Ok(removed)
    }

    /// 把「1 起闭区间」换算成 `Vec` 的 `[lo, hi)` 半开区间，并夹到 `[0, len]`。
    /// 区间无有效内容（`start == 0`、`start > end`、空列表）时返回 `None`。
    fn clamp_line_range(len: usize, start: usize, end: usize) -> Option<(usize, usize)> {
        if start == 0 || start > end || len == 0 {
            return None;
        }
        let lo = (start - 1).min(len);
        let hi = end.min(len);
        (lo < hi).then_some((lo, hi))
    }

    // ============ 全局变量便捷方法 ============

    pub fn set_variable(&mut self, key: impl Into<String>, value: Value) {
        self.global_variables.insert(key.into(), value);
    }

    pub fn get_variable(&self, key: &str) -> Option<&Value> {
        self.global_variables.get(key)
    }

    /// 当前存档的回复台词数量档位；变量缺失或非法一律回落「正常」。
    pub fn reply_length_mode(&self) -> ReplyLengthMode {
        self.get_variable(REPLY_LENGTH_VAR_KEY)
            .and_then(|value| value.as_str())
            .map(ReplyLengthMode::from_str_lossy)
            .unwrap_or_default()
    }

    // ============ 背景（场景 / 剧本临时背景） ============

    /// 当前的剧本临时背景（无记录则为 None）。
    pub fn background_override(&self) -> Option<String> {
        self.get_variable(BACKGROUND_OVERRIDE_VAR_KEY)
            .and_then(|value| value.as_str())
            .map(str::to_string)
    }

    /// 记录剧本临时背景：更新展示背景，并写进全局变量随存档一起持久化。
    /// 路径为空视为没有临时背景，只清掉记录。
    pub fn set_background_override(&mut self, background: String) {
        if background.is_empty() {
            self.global_variables.remove(BACKGROUND_OVERRIDE_VAR_KEY);
        } else {
            self.set_variable(
                BACKGROUND_OVERRIDE_VAR_KEY,
                Value::String(background.clone()),
            );
        }
        self.background = background;
    }

    /// 丢弃剧本临时背景记录。切换当前场景时调用——记录只保留到新场景更新为止。
    pub fn clear_background_override(&mut self) {
        self.global_variables.remove(BACKGROUND_OVERRIDE_VAR_KEY);
    }

    /// 按当前场景刷新展示背景，有临时背景记录时不覆盖。
    ///
    /// 这是「背景跟随场景」的唯一写入口：初始化与读档时场景可能是自动补出来的
    /// （不算用户切了景），所以只同步背景、不动临时记录。
    pub fn sync_background_with_scene(&mut self, scene_background: Option<String>) {
        if self.background_override().is_some() {
            return;
        }
        self.background = scene_background.unwrap_or_default();
    }

    /// 追加一条「提示」台词：旁白 / 系统提示 / 剧情提示。返回它的位置（1 起，
    /// 与 [`Self::read_context_lines`] 的下标口径一致）。
    ///
    /// 这类行有三条不变量，错一条这条提示就废了：
    /// - 属性必须是 USER——SYSTEM 属性会被记忆构建丢弃，提示根本进不了上下文；
    /// - `sender_role_id` 保持 None，从而不计入玩家消息序号、也不算「真实对话」
    ///   （自动存档靠这个判「有没有新剧情」，否则登录提示会把旧档顶掉）；
    /// - 内容必须走 [`PromptRole::build_prompt`]——`{旁白:` 前缀是判定这类行的唯一
    ///   真相源，系统提示词也靠这个信封告诉模型「大括号里的是提示，不是玩家说的话」。
    ///
    /// 与 [`Self::add_line`] 一样是纯追加、不动已有下标，所以生成途中也能插。
    pub async fn add_prompt_line(
        &mut self,
        db: &DatabaseConnection,
        role: PromptRole,
        text: &str,
    ) -> Result<usize> {
        self.add_line(db, PromptLine::new(role, text).into_base())
            .await?;
        Ok(self.line_list.len())
    }

    /// 追加一条「系统提示」台词：给模型的运行时指令，既不是旁白也不是玩家发言。
    /// 等价于 [`Self::add_prompt_line`] 配 [`PromptRole::System`]，不变量见那里。
    pub async fn add_system_hint_line(
        &mut self,
        db: &DatabaseConnection,
        text: &str,
    ) -> Result<()> {
        self.add_prompt_line(db, PromptRole::System, text).await?;
        Ok(())
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

    // ============ 存档状态快照 ============

    /// 将当前 GameStatus 中需要持久化的字段导出为可序列化的快照
    pub fn to_snapshot(&self) -> GameStatusSnapshot {
        GameStatusSnapshot {
            present_role_ids: self.present_role_ids.iter().copied().collect(),
            current_role_id: self.current_role_id,
            background: self.background.clone(),
            background_music: self.background_music.clone(),
            background_effect: self.background_effect.clone(),
            current_scene_id: self.current_scene_id.clone(),
            global_variables: self.global_variables.clone(),
            completed_scripts: self.completed_scripts.iter().cloned().collect(),
            last_dialog_time: self.last_dialog_time.map(|dt| dt.to_rfc3339()),
            scene_awareness_enabled: self.scene_awareness_enabled,
        }
    }

    /// 从快照恢复场景状态
    pub fn apply_snapshot(&mut self, snapshot: &GameStatusSnapshot) {
        self.background = snapshot.background.clone();
        self.background_music = snapshot.background_music.clone();
        self.background_effect = snapshot.background_effect.clone();
        self.current_scene_id = snapshot.current_scene_id.clone();
        self.global_variables = snapshot.global_variables.clone();
        self.completed_scripts = snapshot.completed_scripts.iter().cloned().collect();
        self.last_dialog_time = snapshot.last_dialog_time.as_ref().and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&Local))
        });
        self.current_role_id = snapshot.current_role_id;
        self.present_role_ids = snapshot.present_role_ids.iter().copied().collect();
        self.onstage_role_ids = snapshot.present_role_ids.clone();
        self.scene_awareness_enabled = snapshot.scene_awareness_enabled;
        self.role_manager.overlay_affections_from_vars(
            &self.global_variables,
            self.player.identity_id.as_deref(),
        );
    }
}

/// `GameStatus` 中需要持久化到 `save.status` JSON 的字段。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct GameStatusSnapshot {
    pub present_role_ids: Vec<i32>,
    pub current_role_id: Option<i32>,
    #[serde(default)]
    pub background: String,
    #[serde(default = "default_background_music")]
    pub background_music: String,
    #[serde(default = "default_background_effect")]
    pub background_effect: String,
    #[serde(default)]
    pub current_scene_id: Option<String>,
    #[serde(default)]
    pub global_variables: HashMap<String, Value>,
    #[serde(default)]
    pub completed_scripts: Vec<String>,
    pub last_dialog_time: Option<String>,
    #[serde(default = "default_true")]
    pub scene_awareness_enabled: bool,
}

fn default_true() -> bool {
    true
}

fn default_background_music() -> String {
    "none".into()
}
fn default_background_effect() -> String {
    "none".into()
}
