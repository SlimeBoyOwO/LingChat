use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::{DateTime, Local};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai_service::game_system::role_manager::GameRoleManager;
use crate::ai_service::memory_injection::{
    LineFingerprint, RecentMemory, RecentMemoryAnchor, TemporaryMemory, build_recent_line,
    init_insert_index, recent_insert_index, resolve_recent_index,
};
use crate::ai_service::types::{
    GameLine, GameRole, LineAttributeExt, LineBase, Player, ScriptStatus,
};
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

    /// 试玩会话代号。每次试玩「进来备份 / 走时还原」都会递增；
    /// 消息生成管线在写入台词前比对捕获值与当前值，不一致即视为已过期
    /// （试玩任务被中止后，游离的流式任务可能仍在写）——直接丢弃，保证
    /// 试玩内容不会漏进已还原的自由对话会话。自由对话本身不递增，恒等比对，
    /// 行为不受影响。
    pub preview_generation: u64,

    /// 本会话是否已触发过入场问候（内存标记，重启/清档重置）。
    /// `notify_player_entry` 靠它去重，避免重复生成问候台词；与"玩家是否在游戏里"无关。
    pub entry_greeting_done: bool,

    /// 好感度评估游标：上次评估时「真实对话」的段数。
    /// 每累计 `GodAgentConfig.affection_eval_interval` 段新对话触发一次上帝 Agent 评估。
    pub affection_eval_cursor: usize,

    /// 场景感知开关（关闭后切换场景不再触发旁白）
    pub scene_awareness_enabled: bool,

    /// 生成进行中由插件攒下的旁白，等这一轮收尾、下一轮开头再按顺序落进台词表。
    ///
    /// 生成期间直接 `add_prompt_line` 会往流式任务算好的行号后面插队（它按位置回填
    /// 助手台词），事件一多就会与正在生成的这一轮搅在一起。排队到下一轮开头写，
    /// 既保住行号，也让「这一轮生成时发生的事」完整地出现在下一轮上下文里。
    /// 不参与存档：换会话时随 `GameStatus` 一起丢弃。
    pending_prompt_lines: Vec<QueuedPromptLine>,

    /// 待落位的近期记忆（好感度这类后台任务产出），不参与存档。
    pending_recent_memories: Vec<RecentMemory>,

    /// 待注入下一轮请求的临时记忆，取用后即丢，不写台词表、不参与存档。
    pending_temporary_memories: Vec<TemporaryMemory>,

    /// 回复后工具推荐的暂存：下一轮请求取走并渲染成临时提示，取用后即丢。
    /// 每轮用户发言前若又算出了新的推荐，它会被覆盖——同一时刻只保留最后一份，
    /// 避免两份推荐提示同时出现在请求里。
    pending_tool_hint: Option<Vec<String>>,
}

/// 一条等下一轮开头再写的插件旁白。
pub struct QueuedPromptLine {
    role: PromptRole,
    text: String,
    /// 进队时刻，用来判「轮到时还值不值得说」。
    queued_at: Instant,
    /// 有效期；`None` = 不参与时效判定（入场提示这类，写进去就一直算数）。
    ttl: Option<Duration>,
    /// 同一类事件的合并键（如受伤）。队列里已有同键条目时保留更严重的一条。
    key: Option<String>,
    /// 严重度，同键合并时用；越大越值得留下。
    severity: i64,
}

/// 排队上限。战斗里每挨一下都可能攒一条，上限到了丢最早的：最近发生的事更重要。
const MAX_PENDING_PROMPT_LINES: usize = 200;

/// 待落位的近期记忆上限，只防异常堆积。
const MAX_PENDING_RECENT_MEMORIES: usize = 64;

/// 排队旁白接成一行的长度上限；再长就另起一行（与插件侧的合并口径各自兜底）。
const MAX_QUEUED_LINE_CHARS: usize = 600;

/// 丢掉超过有效期（`ttl`）的排队旁白。只动队列，不碰台词表。
fn drop_expired_prompt_lines(queue: &mut Vec<QueuedPromptLine>, now: Instant) -> usize {
    let before = queue.len();
    queue.retain(|line| {
        let Some(ttl) = line.ttl else {
            return true;
        };
        now.saturating_duration_since(line.queued_at) <= ttl
    });
    let dropped = before - queue.len();
    if dropped > 0 {
        tracing::info!("[GameStatus] 丢弃已过期的插件旁白 {dropped} 条");
    }
    dropped
}

/// 入队一条旁白，处理「同类事件只留最严重的一条」与「连续旁白接成一行」。
fn enqueue_prompt_line(
    queue: &mut Vec<QueuedPromptLine>,
    role: PromptRole,
    text: String,
    ttl: Option<Duration>,
    key: Option<String>,
    severity: i64,
    now: Instant,
) {
    let key = key.filter(|value| !value.trim().is_empty());
    if let Some(key) = key.as_deref() {
        // 同一件事的连续报告只留最严重的一条：连着挨三下不该记成三条旁白，
        // 但也不能因为最后一次挨得轻就把前面那下重的抹掉。
        if let Some(existing) = queue
            .iter_mut()
            .find(|line| line.key.as_deref() == Some(key))
        {
            if severity >= existing.severity {
                existing.text = text;
                existing.queued_at = now;
                existing.ttl = ttl;
                existing.severity = severity;
            }
            return;
        }
    }

    if ttl.is_none() {
        if let Some(last) = queue.last_mut() {
            if matches!(
                (&last.role, &role),
                (PromptRole::Narrator, PromptRole::Narrator)
            ) && last.ttl.is_none()
                && last.key.is_none()
                && last.text.len() + text.len() < MAX_QUEUED_LINE_CHARS
            {
                last.text.push('\n');
                last.text.push_str(&text);
                return;
            }
        }
    }
    if queue.len() >= MAX_PENDING_PROMPT_LINES {
        queue.remove(0);
        tracing::warn!(
            "[GameStatus] 插件旁白积压到上限（{} 条），丢弃最早的一条",
            MAX_PENDING_PROMPT_LINES
        );
    }
    queue.push(QueuedPromptLine {
        role,
        text,
        queued_at: now,
        ttl,
        key,
        severity,
    });
}

/// 队列里带时效（游戏事件）的条目数；入场提示这类不计数。
fn timed_prompt_count(queue: &[QueuedPromptLine]) -> usize {
    queue.iter().filter(|line| line.ttl.is_some()).count()
}

/// 把队列排成待写的行：连续的 narrator 旁白用换行接成一行，其余各占一条。
fn take_prompt_batches(queue: Vec<QueuedPromptLine>) -> Vec<(PromptRole, String)> {
    let mut batches: Vec<(PromptRole, String)> = Vec::new();
    for line in queue {
        if let Some(last) = batches.last_mut() {
            if matches!(
                (&last.0, &line.role),
                (PromptRole::Narrator, PromptRole::Narrator)
            ) && last.1.len() + line.text.len() < MAX_QUEUED_LINE_CHARS
            {
                last.1.push('\n');
                last.1.push_str(&line.text);
                continue;
            }
        }
        batches.push((line.role, line.text));
    }
    batches
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
            pending_prompt_lines: Vec::new(),
            pending_recent_memories: Vec::new(),
            pending_temporary_memories: Vec::new(),
            pending_tool_hint: None,
        }
    }

    /// 攒一条插件旁白，等下一轮开头由 [`Self::flush_pending_prompt_lines`] 写进台词表。
    ///
    /// `ttl` 与 `key` 是给游戏事件用的：有有效期的条目**不会立刻进台词表**，等到真的
    /// 要开口那一刻再判一次——已经过期的直接丢掉，不让一分钟前捡起一块木头这种事
    /// 混进此刻的对话。`key` 相同的条目（比如连着挨打）只留 `severity` 最大的那条。
    ///
    /// 连着来的旁白会先接成一行（换行隔开）：同一批事件拆成好几行落进台词表，
    /// 模型会把它们当成好几件先后发生的事来复述。系统 / 剧情提示不并。
    ///
    /// 返回队列里还有效的带时效条目数，插件用它判「攒够几条了」。
    pub fn queue_prompt_line(
        &mut self,
        role: PromptRole,
        text: impl Into<String>,
        ttl: Option<Duration>,
        key: Option<String>,
        severity: i64,
    ) -> usize {
        let now = Instant::now();
        drop_expired_prompt_lines(&mut self.pending_prompt_lines, now);
        enqueue_prompt_line(
            &mut self.pending_prompt_lines,
            role,
            text.into(),
            ttl,
            key,
            severity,
            now,
        );
        timed_prompt_count(&self.pending_prompt_lines)
    }

    /// 把攒下的插件旁白按顺序写进台词表。
    ///
    /// **必须在持有 `generation_lock` 之后、构建本轮上下文之前调用**，否则又会和
    /// 流式写入抢行号。写之前先按有效期筛一遍：过期的游戏事件直接丢掉，不进台词表；
    /// 连续的 narrator 旁白仍接成一行（换行隔开）。
    /// 返回实际写入的条数（写入失败的条目不阻塞后面的）。
    pub async fn flush_pending_prompt_lines(&mut self, db: &DatabaseConnection) -> usize {
        drop_expired_prompt_lines(&mut self.pending_prompt_lines, Instant::now());
        if self.pending_prompt_lines.is_empty() {
            return 0;
        }
        let queued = std::mem::take(&mut self.pending_prompt_lines);
        let batches = take_prompt_batches(queued);
        let mut written = 0;
        for (role, text) in batches {
            match self.add_prompt_line(db, role, &text).await {
                Ok(_) => written += 1,
                Err(e) => tracing::warn!("[GameStatus] 补写插件旁白失败：{e}"),
            }
        }
        if written > 0 {
            tracing::info!("[GameStatus] 补写排队中的插件旁白 {written} 条");
        }
        written
    }

    /// 用指定下标上的行构造锚点；该行不是 User 行时返回 None。
    /// 评估时调用，此时玩家行的临时消息段已清理，指纹取到的是最终内容。
    pub fn recent_anchor_at(&self, index: usize) -> Option<RecentMemoryAnchor> {
        let line = self.line_list.get(index)?;
        if !matches!(line.attribute(), LineAttribute::User) {
            return None;
        }
        let mut perceived_role_ids: Vec<i32> = self
            .present_role_ids
            .iter()
            .copied()
            .filter(|id| *id != 0)
            .collect();
        perceived_role_ids.sort_unstable();
        Some(RecentMemoryAnchor {
            index,
            fingerprint: LineFingerprint::of(line),
            session: self.preview_generation,
            perceived_role_ids,
        })
    }

    /// 末尾最后一条 User 行的下标，供调用方确定本轮锚点。
    pub fn last_user_line_index(&self) -> Option<usize> {
        self.line_list
            .len()
            .checked_sub(1)
            .filter(|&index| matches!(self.line_list[index].attribute(), LineAttribute::User))
    }

    /// 入队一条近期记忆，等本轮结束或下一轮开头落位。
    pub fn queue_recent_memory(&mut self, memory: RecentMemory) {
        if self.pending_recent_memories.len() >= MAX_PENDING_RECENT_MEMORIES {
            self.pending_recent_memories.remove(0);
        }
        self.pending_recent_memories.push(memory);
    }

    /// 清空待落位的近期记忆。换档/清档时调用，避免旧会话的产出延迟写入。
    pub fn clear_pending_recent_memories(&mut self) {
        self.pending_recent_memories.clear();
    }

    /// 入队一条只在下一轮请求出现的临时记忆，文本重复时不再入队。
    pub fn queue_temporary_memory(&mut self, memory: TemporaryMemory) {
        if self
            .pending_temporary_memories
            .iter()
            .any(|pending| pending.text == memory.text)
        {
            return;
        }
        self.pending_temporary_memories.push(memory);
    }

    /// 取走全部临时记忆，只用于构建一次请求上下文。
    pub fn take_temporary_memories(&mut self) -> Vec<TemporaryMemory> {
        std::mem::take(&mut self.pending_temporary_memories)
    }

    pub fn clear_pending_temporary_memories(&mut self) {
        self.pending_temporary_memories.clear();
    }

    /// 暂存回复后推荐的工具名，覆盖上一份。
    pub fn set_pending_tool_hint(&mut self, names: Vec<String>) {
        self.pending_tool_hint = (!names.is_empty()).then_some(names);
    }

    /// 取走暂存的推荐工具名，只用于构建一次请求上下文。
    pub fn take_pending_tool_hint(&mut self) -> Option<Vec<String>> {
        self.pending_tool_hint.take()
    }

    /// 查看暂存的推荐工具名（调试指令用，不取走）。
    pub fn pending_tool_hint(&self) -> Option<&[String]> {
        self.pending_tool_hint.as_deref()
    }

    pub fn clear_pending_tool_hint(&mut self) {
        self.pending_tool_hint = None;
    }

    /// 把待落位的近期记忆写进台词表：锚点之后连续 User 块的末尾。
    /// 锚点失效或本轮没有 user 行时丢弃，返回实际写入条数。
    pub async fn flush_recent_memories(&mut self, db: &DatabaseConnection) -> usize {
        if self.script_status.is_some() {
            self.pending_recent_memories.clear();
            tracing::warn!("[Memory] 剧本模式下丢弃待落位的近期记忆");
            return 0;
        }

        let pending: Vec<RecentMemory> = std::mem::take(&mut self.pending_recent_memories)
            .into_iter()
            .filter(|memory| {
                memory
                    .anchor
                    .as_ref()
                    .map_or(true, |anchor| anchor.session == self.preview_generation)
            })
            .collect();
        if pending.is_empty() {
            return 0;
        }

        let has_dialogue = self.line_list.iter().any(|line| {
            matches!(
                line.attribute(),
                LineAttribute::User | LineAttribute::Assistant
            )
        });
        let mut current_perceived: Vec<i32> = self
            .present_role_ids
            .iter()
            .copied()
            .filter(|id| *id != 0)
            .collect();
        current_perceived.sort_unstable();

        // 先解析全部插入点，再按下标从大到小插入，避免前一次插入让后一次下标漂移。
        let mut planned: Vec<(usize, GameLine)> = Vec::new();
        for memory in pending {
            let (insert_at, perceived) = match &memory.anchor {
                Some(anchor) => {
                    let Some(anchor_index) = resolve_recent_index(&self.line_list, anchor) else {
                        tracing::warn!("[Memory] 近期记忆锚点已失效，丢弃");
                        continue;
                    };
                    (
                        recent_insert_index(&self.line_list, anchor_index),
                        anchor.perceived_role_ids.clone(),
                    )
                },
                None if !has_dialogue => (
                    init_insert_index(&self.line_list),
                    current_perceived.clone(),
                ),
                None => {
                    tracing::warn!("[Memory] 本轮没有 user 台词，丢弃近期记忆");
                    continue;
                },
            };
            planned.push((insert_at, build_recent_line(&memory, perceived)));
        }
        if planned.is_empty() {
            return 0;
        }

        self.role_manager.invalidate_memory_history();
        planned.sort_by_key(|(index, _)| *index);
        let min_index = planned[0].0;
        let written = planned.len();
        for (index, line) in planned.into_iter().rev() {
            let at = index.min(self.line_list.len());
            self.line_list.insert(at, line);
        }
        // 插入点早于持久记忆的压缩指针时回拨，避免摘要区和原文重复。
        self.role_manager.rewind_memory_pointers(min_index).await;
        if let Err(e) = self.refresh_memories(db).await {
            tracing::warn!("[Memory] 近期记忆写入后刷新记忆失败: {e:#}");
        }
        written
    }

    pub async fn get_role<'a>(
        &'a mut self,
        db: &DatabaseConnection,
        role_id: i32,
    ) -> Result<&'a mut GameRole> {
        let var = self
            .global_variables
            .get(&crate::ai_service::affection::var_key(role_id))
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
        let game_line = GameLine::from_base(line, perceived);
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

    // ── 台词历史（上下文源）的区间查看 / 编辑 ──
    //
    // 「第 start~end 条」一律指 `line_list` 的下标：**从 1 开始、闭区间**，第 1 条
    // 通常就是 role system 的人设行；越界会被夹到可用范围。这里操作的是台词行
    // （`GameLine`，全字段），**不是**渲染后发给 LLM 的 `LlmMessage`——渲染会按角色
    // 合并/过滤，两者不是同一套下标空间。
    //
    // 参数名用 `start`/`end` 而非 `from`/`to`：与插件侧同名 API 对齐（Python 里
    // `from` 是关键字，用不了）。

    /// 读取第 `start`~`end` 条台词（含两端，1 起）的完整运行时行。
    ///
    /// 返回的 `GameLine` 携带全部字段（情绪、动作、TTS、音频、thinking、tool_call、
    /// 感知集合等），可改后原样回传给 [`Self::edit_context_lines`]；只读，无副作用。
    pub fn read_context_lines(&self, start: usize, end: usize) -> Vec<GameLine> {
        match Self::clamp_line_range(self.line_list.len(), start, end) {
            Some((lo, hi)) => self.line_list[lo..hi].to_vec(),
            None => Vec::new(),
        }
    }

    /// 用 `replacement` 替换第 `start`~`end` 条台词（含两端，1 起），并让改动真正
    /// 落进上下文。`replacement` 条数不限：空 = 删除该区间，多条 = 展开。
    ///
    /// 依次做：
    /// - 生成进行中直接拒绝（`generation_lock` 被占），避免改历史与流式写入打架；
    /// - System 人设行可编辑/删除：删掉后相应角色的上下文失去 system 前缀，只记
    ///   一条 warn，不拦截；
    /// - `replacement` 中未带 id 的行按位置继承被替换行的 id，保住存档链锚点
    ///   （否则 `SaveRepo::sync_lines` 可能因「首行分歧」拒绝覆盖）；
    /// - 回拨所有角色的压缩指针到编辑起点（仅当指针在其之后），使被编辑的早期
    ///   内容重新进入窗口、下次压缩重新摘要；
    /// - `invalidate_memory_history` + `refresh_memories`。
    ///
    /// 返回被替换掉的条数。**落库不在这里**，由调用方走正常存盘流程
    /// （`SaveRepo::sync_lines`）持久化。
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
        self.add_line(
            db,
            LineBase {
                content: role.build_prompt(text),
                attribute: LineAttributeExt(LineAttribute::User),
                display_name: Some(role.display_name().to_string()),
                ..Default::default()
            },
        )
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
        self.role_manager
            .overlay_affections_from_vars(&self.global_variables);
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

#[cfg(test)]
mod queue_tests {
    use super::*;

    fn event(
        text: &str,
        ttl_secs: u64,
        key: &str,
        severity: i64,
        now: Instant,
    ) -> QueuedPromptLine {
        QueuedPromptLine {
            role: PromptRole::Narrator,
            text: text.to_string(),
            queued_at: now,
            ttl: Some(Duration::from_secs(ttl_secs)),
            key: (!key.is_empty()).then(|| key.to_string()),
            severity,
        }
    }

    #[test]
    fn expired_events_are_dropped_from_the_queue() {
        let now = Instant::now();
        let mut queue = vec![
            event("捡起了木头", 5, "", 0, now),
            event("挨了僵尸一下", 60, "", 0, now),
        ];
        let later = now + Duration::from_secs(10);
        assert_eq!(drop_expired_prompt_lines(&mut queue, later), 1);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].text, "挨了僵尸一下");
    }

    #[test]
    fn no_ttl_entries_never_expire() {
        let now = Instant::now();
        let mut queue = vec![QueuedPromptLine {
            role: PromptRole::Narrator,
            text: "进入 MC 的提示".into(),
            queued_at: now,
            ttl: None,
            key: None,
            severity: 0,
        }];
        drop_expired_prompt_lines(&mut queue, now + Duration::from_secs(3600));
        assert_eq!(queue.len(), 1);
        assert_eq!(timed_prompt_count(&queue), 0);
    }

    #[test]
    fn same_key_keeps_the_most_severe_line() {
        let now = Instant::now();
        let mut queue = vec![event("伤得不轻", 60, "hurt", 800, now)];
        // 轻的一下不该覆盖重的
        enqueue_prompt_line(
            &mut queue,
            PromptRole::Narrator,
            "受了点轻伤".into(),
            Some(Duration::from_secs(60)),
            Some("hurt".into()),
            100,
            now + Duration::from_secs(1),
        );
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].text, "伤得不轻");
        // 更重的一下替换掉原来的
        enqueue_prompt_line(
            &mut queue,
            PromptRole::Narrator,
            "伤得很重".into(),
            Some(Duration::from_secs(60)),
            Some("hurt".into()),
            1500,
            now + Duration::from_secs(2),
        );
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].text, "伤得很重");
    }

    #[test]
    fn consecutive_narrator_events_become_one_line() {
        let now = Instant::now();
        let mut queue = vec![
            event("天黑了", 300, "", 0, now),
            event("开始下雨了", 300, "", 0, now),
        ];
        let batches = take_prompt_batches(queue.split_off(0));
        assert_eq!(batches.len(), 1);
        assert!(matches!(batches[0].0, PromptRole::Narrator));
        assert_eq!(batches[0].1, "天黑了\n开始下雨了");
    }
}
