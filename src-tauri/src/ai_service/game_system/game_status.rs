use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{DateTime, Local};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::types::{
    GameLine, GameRole, LineAttributeExt, LineBase, Player, ScriptStatus,
};
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

/// Canonical-history change semantics consumed by the memory coordinator.
///
/// Appends preserve an already captured compression prefix. Any rewrite invalidates
/// in-flight work; a rewrite that touches a processed prefix also resets that
/// role's permanent bank before context is rebuilt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryChange {
    Append {
        from_idx: usize,
    },
    Rewrite {
        from_idx: usize,
    },
    ReplaceAll,
    /// A canonical-history mutation confined to an editor preview. It rebuilds
    /// preview short-term contexts only: it must neither reset nor commit the
    /// formal session's unique MemoryBank.
    Preview,
    /// Rebuild every loaded context after a matching MemoryBank snapshot was
    /// restored. Unlike ReplaceAll, the runtime was already reset from that
    /// snapshot and must not discard it again.
    Restore,
}

/// Identity of the canonical history a producer was admitted to write.
///
/// A preview transition increments `generation` both when entering and when
/// restoring. `mode` is retained as a separate invariant so a formal producer
/// cannot write into an active preview even if a future generation policy
/// changes. Callers capture this before their first await and use one of the
/// conditional mutation APIs below at every canonical write point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistorySession {
    pub generation: u64,
    pub is_preview: bool,
}

impl HistoryChange {
    pub fn rewrite_from(self) -> Option<usize> {
        match self {
            Self::Append { .. } => None,
            Self::Rewrite { from_idx } => Some(from_idx),
            Self::ReplaceAll => Some(0),
            Self::Preview | Self::Restore => None,
        }
    }

    pub fn append_from(self) -> Option<usize> {
        match self {
            Self::Append { from_idx } => Some(from_idx),
            Self::Rewrite { .. } | Self::ReplaceAll | Self::Preview | Self::Restore => None,
        }
    }
}

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

    /// 试玩会话代号。每次试玩「进来备份 / 走时还原」都会递增；
    /// 消息生成管线在写入台词前比对捕获值与当前值，不一致即视为已过期
    /// （试玩任务被中止后，游离的流式任务可能仍在写）——直接丢弃，保证
    /// 试玩内容不会漏进已还原的自由对话会话。自由对话本身不递增，恒等比对，
    /// 行为不受影响。
    pub preview_generation: u64,

    /// Linearizes preview transitions with every formal-session persistence
    /// operation. It is intentionally an async mutex: callers never hold a
    /// synchronous runtime-state lock across I/O, while a preview transition
    /// and its corresponding formal write have one unambiguous order.
    preview_session_gate: std::sync::Arc<tokio::sync::Mutex<()>>,

    /// 标记玩家是否已在本会话中入场（内存标记，重启重置）。
    /// 用于防止重复触发入场问候。
    pub player_entered: bool,

    /// 场景感知开关（关闭后切换场景不再触发旁白）
    pub scene_awareness_enabled: bool,
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
            preview_session_gate: std::sync::Arc::new(tokio::sync::Mutex::new(())),
            player_entered: false,
            scene_awareness_enabled: true,
        }
    }

    /// Clone the common preview/formal-save gate. Callers must acquire this
    /// before the GameStatus mutex; PreviewSession and AIService use the same
    /// order, preventing a check-then-write preview race.
    pub fn preview_session_gate(&self) -> std::sync::Arc<tokio::sync::Mutex<()>> {
        self.preview_session_gate.clone()
    }

    /// Capture the current canonical-history identity for an asynchronous
    /// producer. The returned value must be verified through a conditional
    /// mutation API rather than by a separate check followed by `add_line`.
    pub fn history_session(&self) -> HistorySession {
        HistorySession {
            generation: self.preview_generation,
            is_preview: self.role_manager.is_memory_preview(),
        }
    }

    /// Verify an already captured history identity while the caller holds the
    /// `GameStatus` mutex. This is intended for async producers whose analysis
    /// yielded no canonical line: they still need to reject a session switch
    /// before they start downstream LLM, tool, memory, or save work.
    pub fn is_history_session_current(&self, expected: HistorySession) -> bool {
        self.history_session() == expected
    }

    /// Admit one potentially irreversible tool execution for a captured
    /// history identity. The common preview gate is acquired before
    /// `GameStatus`, then retained by the caller through the tool await.
    /// Consequently Preview cannot slip between admission and execution;
    /// callers must not retain the `GameStatus` mutex while holding this permit.
    pub async fn admit_tool_execution_if_current(
        game_status: &std::sync::Arc<tokio::sync::Mutex<Self>>,
        expected: HistorySession,
    ) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        let gate = game_status.lock().await.preview_session_gate();
        let permit = gate.lock_owned().await;
        if game_status
            .lock()
            .await
            .is_history_session_current(expected)
        {
            Some(permit)
        } else {
            None
        }
    }

    fn matches_history_session(&self, expected: HistorySession) -> bool {
        self.is_history_session_current(expected)
    }

    pub async fn get_role<'a>(
        &'a mut self,
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<&'a mut GameRole> {
        self.role_manager.get_role(db, role_id).await
    }

    /// Append one canonical line and refresh only roles affected by that append.
    /// An append deliberately does not invalidate a compression that already
    /// captured an earlier target index.
    pub async fn append_line(&mut self, db: &DatabaseConnection, line: LineBase) -> Result<()> {
        let from_idx = self.line_list.len();
        let perceived: Vec<i32> = self.present_role_ids.iter().copied().collect();
        self.line_list.push(GameLine::from_base(line, perceived));
        self.refresh_memories_for_change(db, HistoryChange::Append { from_idx })
            .await
    }

    /// Atomically verify a producer's captured session and append one line.
    ///
    /// This is deliberately narrow: the caller already owns the `GameStatus`
    /// mutex, and the identity check plus canonical mutation run in that same
    /// critical section. A stale producer returns `Ok(false)` without memory
    /// refresh, compression admission, or persistence side effects.
    pub async fn append_line_if_current(
        &mut self,
        db: &DatabaseConnection,
        expected: HistorySession,
        line: LineBase,
    ) -> Result<bool> {
        if !self.matches_history_session(expected) {
            return Ok(false);
        }
        self.append_line(db, line).await?;
        Ok(true)
    }

    /// Compatibility spelling for existing callers. New code should use
    /// [`Self::append_line`] so the mutation intent stays explicit.
    pub async fn add_line(&mut self, db: &DatabaseConnection, line: LineBase) -> Result<()> {
        self.append_line(db, line).await
    }

    /// Append a batch and rebuild contexts once, rather than once per line.
    pub async fn append_lines<I>(&mut self, db: &DatabaseConnection, lines: I) -> Result<()>
    where
        I: IntoIterator<Item = LineBase>,
    {
        let from_idx = self.line_list.len();
        let perceived: Vec<i32> = self.present_role_ids.iter().copied().collect();
        self.line_list.extend(
            lines
                .into_iter()
                .map(|line| GameLine::from_base(line, perceived.clone())),
        );
        if self.line_list.len() != from_idx {
            self.refresh_memories_for_change(db, HistoryChange::Append { from_idx })
                .await?;
        }
        Ok(())
    }

    /// Insert canonical lines into history. Mid-history insertion is a rewrite,
    /// never an append, because an existing MemoryBank may have summarized the
    /// shifted suffix (tool-result backfill uses this path).
    pub async fn insert_lines<I>(
        &mut self,
        db: &DatabaseConnection,
        insert_idx: usize,
        lines: I,
    ) -> Result<()>
    where
        I: IntoIterator<Item = LineBase>,
    {
        let insert_idx = insert_idx.min(self.line_list.len());
        let perceived: Vec<i32> = self.present_role_ids.iter().copied().collect();
        let inserted: Vec<GameLine> = lines
            .into_iter()
            .map(|line| GameLine::from_base(line, perceived.clone()))
            .collect();
        if inserted.is_empty() {
            return Ok(());
        }
        self.line_list.splice(insert_idx..insert_idx, inserted);
        self.refresh_memories_for_change(
            db,
            HistoryChange::Rewrite {
                from_idx: insert_idx,
            },
        )
        .await
    }

    /// Atomically verify a producer's captured session and insert canonical
    /// lines. Like `append_line_if_current`, a mismatch is a no-op with no
    /// memory side effects.
    pub async fn insert_lines_if_current<I>(
        &mut self,
        db: &DatabaseConnection,
        expected: HistorySession,
        insert_idx: usize,
        lines: I,
    ) -> Result<bool>
    where
        I: IntoIterator<Item = LineBase>,
    {
        if !self.matches_history_session(expected) {
            return Ok(false);
        }
        self.insert_lines(db, insert_idx, lines).await?;
        Ok(true)
    }

    /// Truncate canonical history and invalidate/reset affected permanent memory.
    pub async fn truncate_lines(&mut self, db: &DatabaseConnection, from_idx: usize) -> Result<()> {
        let from_idx = from_idx.min(self.line_list.len());
        self.line_list.truncate(from_idx);
        self.refresh_memories_for_change(db, HistoryChange::Rewrite { from_idx })
            .await
    }

    /// Replace the complete canonical history, e.g. while loading a save.
    pub async fn replace_lines(
        &mut self,
        db: &DatabaseConnection,
        lines: Vec<GameLine>,
    ) -> Result<()> {
        self.line_list = lines;
        self.refresh_memories_for_change(db, HistoryChange::ReplaceAll)
            .await
    }

    /// Notify permanent memory after an in-place canonical-history rewrite.
    /// Callers that directly edit a line must provide its earliest changed index.
    pub async fn line_changed(&mut self, db: &DatabaseConnection, from_idx: usize) -> Result<()> {
        self.refresh_memories_for_change(db, HistoryChange::Rewrite { from_idx })
            .await
    }

    pub async fn refresh_memories_for_change(
        &mut self,
        db: &DatabaseConnection,
        change: HistoryChange,
    ) -> Result<()> {
        // This is the sole preview-history boundary. All named line mutation
        // APIs flow here, so rollback/truncate and mid-history tool backfill
        // rebuild the preview context without applying their normal Rewrite
        // semantics to the formal runtime. Entering Preview already invalidates
        // any admitted formal job; Preview itself never starts a new one.
        let change = if self.role_manager.is_memory_preview() {
            HistoryChange::Preview
        } else {
            change
        };
        self.role_manager
            .sync_memories(db, &self.line_list, change)
            .await
    }

    /// Rebuild contexts after a MemoryBank snapshot was restored from the same
    /// save. The restore already invalidated old jobs and installed the matching
    /// permanent bank, so this must not reset that bank a second time.
    pub async fn rebuild_memories_after_restore(&mut self, db: &DatabaseConnection) -> Result<()> {
        self.refresh_memories_for_change(db, HistoryChange::Restore)
            .await
    }

    /// Install preview canonical history without changing the normal session's
    /// permanent bank. Preview mode must already be active; all mutations while
    /// it remains active are translated to `HistoryChange::Preview` by the
    /// common boundary above.
    pub async fn replace_preview_history(
        &mut self,
        db: &DatabaseConnection,
        lines: Vec<GameLine>,
    ) -> Result<()> {
        self.line_list = lines;
        self.refresh_memories_for_change(db, HistoryChange::Preview)
            .await
    }

    /// Restore the formal canonical history and leave preview mode as one
    /// boundary. Contexts rebuild while Preview is still active, so the restore
    /// cannot itself schedule a formal compaction or mutate bank/revision.
    /// Subsequent normal-session appends resume ordinary compression.
    pub async fn restore_preview_history(
        &mut self,
        db: &DatabaseConnection,
        lines: Vec<GameLine>,
    ) -> Result<()> {
        let result = self.replace_preview_history(db, lines).await;
        self.role_manager.set_memory_preview(false);
        result
    }

    /// Rebuild all role contexts without changing canonical history. This is
    /// for role/scene changes only; callers that mutate `line_list` must use a
    /// named history API above so permanent-memory invalidation is not skipped.
    pub async fn refresh_memories(&mut self, db: &DatabaseConnection) -> Result<()> {
        self.refresh_memories_for_change(db, HistoryChange::Append { from_idx: 0 })
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

    pub async fn add_character_clothes_change_line(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        clothes_name: &str,
    ) -> Result<()> {
        let role = self
            .role_manager
            .get_loaded_mut(role_id)
            .ok_or_else(|| anyhow::anyhow!("角色 {} 未加载", role_id))?;

        role.current_clothes = clothes_name.to_string();

        let ai_name = role.settings.ai_name.clone();
        let clothes_prompt = role
            .settings
            .clothes
            .as_ref()
            .and_then(|list| {
                list.iter().find_map(|item| {
                    if item.get("name").map(|s| s.as_str()) == Some(clothes_name) {
                        item.get("prompt").cloned()
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();

        let prompt = format!(
            "{}换上了新服装：{}，{}",
            ai_name, clothes_name, clothes_prompt
        );

        self.add_line(
            db,
            LineBase {
                content: PromptRole::Narrator.build_prompt(&prompt),
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some("旁白".to_string()),
                ..Default::default()
            },
        )
        .await
        .map_err(|e| anyhow::anyhow!("添加换装台词失败: {}", e))?;

        Ok(())
    }

    /// 切换角色服装并生成旁白台词。
    /// 若已是目标服装则跳过。返回是否实际切换。
    pub async fn on_character_change_clothes(
        &mut self,
        db: &DatabaseConnection,
        role_id: i32,
        clothes_name: &str,
    ) -> Result<bool> {
        let role = self
            .role_manager
            .get_loaded_mut(role_id)
            .ok_or_else(|| anyhow::anyhow!("角色 {} 未加载", role_id))?;

        if role.current_clothes == clothes_name {
            return Ok(false);
        }

        self.add_character_clothes_change_line(db, role_id, clothes_name)
            .await?;

        Ok(true)
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
