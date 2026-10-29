//! 待落位队列：生成进行中不能直接写台词表时，把提示行先攒下来。
//!
//! 两条队列对应两种落点：
//! - `prompt_lines`：插件旁白，落位时追加到台词表末尾（可带有效期与合并键）；
//! - `recent_memories`：好感度这类后台产出，落位时锚定到某轮 user 台词块末尾。
//!
//! 两边都会在落位前过一遍合并规则，写进台词表的内容由 [`take_prompt_batches`] 与
//! 锚点算法决定；入队阶段只做去重与上限定容，不拼接文本。

use std::time::{Duration, Instant};

use crate::ai_service::types::GameLine;
use crate::db::entities::line::LineAttribute;
use crate::utils::prompt::PromptRole;

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

/// 等下一轮落位的暂存。
#[derive(Default)]
pub struct Pending {
    /// 生成进行中由插件攒下的旁白。
    pub(super) prompt_lines: Vec<QueuedPromptLine>,
    /// 待落位的近期记忆（好感度这类后台任务产出），不参与存档。
    pub(super) recent_memories: Vec<RecentMemory>,
}

impl Pending {
    /// 入队一条旁白，返回队列里还有效的带时效条目数。
    pub(super) fn queue_prompt_line(
        &mut self,
        role: PromptRole,
        text: String,
        ttl: Option<Duration>,
        key: Option<String>,
        severity: i64,
    ) -> usize {
        let now = Instant::now();
        drop_expired(&mut self.prompt_lines, now);
        enqueue(&mut self.prompt_lines, role, text, ttl, key, severity, now);
        self.prompt_lines
            .iter()
            .filter(|line| line.ttl.is_some())
            .count()
    }

    /// 取走排队的旁白，按合并规则排成待写的行。
    pub(super) fn take_prompt_batches(&mut self) -> Vec<(PromptRole, String)> {
        drop_expired(&mut self.prompt_lines, Instant::now());
        if self.prompt_lines.is_empty() {
            return Vec::new();
        }
        take_prompt_batches(std::mem::take(&mut self.prompt_lines))
    }

    /// 入队一条近期记忆，超出上限丢最早的。
    pub(super) fn queue_recent_memory(&mut self, memory: RecentMemory) {
        if self.recent_memories.len() >= MAX_PENDING_RECENT_MEMORIES {
            self.recent_memories.remove(0);
        }
        self.recent_memories.push(memory);
    }

    /// 取走全部待落位的近期记忆。
    pub(super) fn take_recent_memories(&mut self) -> Vec<RecentMemory> {
        std::mem::take(&mut self.recent_memories)
    }

    pub(super) fn clear_recent_memories(&mut self) {
        self.recent_memories.clear();
    }
}

/// 丢掉超过有效期（`ttl`）的排队旁白。只动队列，不碰台词表。
fn drop_expired(queue: &mut Vec<QueuedPromptLine>, now: Instant) -> usize {
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

/// 入队一条旁白：同类事件只留最严重的一条，超出上限丢最早的。
///
/// 连续旁白不在这里拼接——落位时由 [`take_prompt_batches`] 统一合并，避免同一条
/// 规则写两遍。
fn enqueue(
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

// ============================================================
// 近期记忆：锚定到某一轮 user 台词
// ============================================================

/// 写进台词表的一条提示/旁白。
#[derive(Clone, Debug)]
pub struct RecentMemory {
    pub text: String,
    pub role: PromptRole,
    /// 目标轮次的锚点；`None` 表示初始化阶段没有 user 行可挂。
    pub anchor: Option<RecentMemoryAnchor>,
}

/// 锚定到某一轮 user 行的锚点。
#[derive(Clone, Debug)]
pub struct RecentMemoryAnchor {
    pub index: usize,
    /// 下标漂移后用来确认还是同一行。
    pub fingerprint: LineFingerprint,
    /// 捕获时的会话代号（`GameStatus::preview_generation`）。
    pub session: u64,
    /// 捕获时的在场角色，写入行的感知集合。
    pub perceived_role_ids: Vec<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineFingerprint {
    pub content: String,
    pub attribute: LineAttribute,
    pub sender_role_id: Option<i32>,
    pub display_name: Option<String>,
}

impl LineFingerprint {
    pub fn of(line: &GameLine) -> Self {
        Self {
            content: line.base.content.clone(),
            attribute: line.base.attribute.inner().clone(),
            sender_role_id: line.base.sender_role_id,
            display_name: line.base.display_name.clone(),
        }
    }

    pub fn matches(&self, line: &GameLine) -> bool {
        self.attribute == *line.base.attribute.inner()
            && self.sender_role_id == line.base.sender_role_id
            && self.display_name == line.base.display_name
            && self.content == line.base.content
    }
}

/// 锚点搜索窗口：中段插入会让下标前后漂移，在这个范围内按指纹兜底。
const ANCHOR_SEARCH_WINDOW: usize = 32;

/// 定位锚点行：先按下标命中，不中再在前后窗口内按指纹找。
pub(super) fn resolve_recent_index(
    lines: &[GameLine],
    anchor: &RecentMemoryAnchor,
) -> Option<usize> {
    if lines
        .get(anchor.index)
        .is_some_and(|line| anchor.fingerprint.matches(line))
    {
        return Some(anchor.index);
    }

    for distance in 1..=ANCHOR_SEARCH_WINDOW {
        if let Some(index) = anchor.index.checked_sub(distance) {
            if lines
                .get(index)
                .is_some_and(|line| anchor.fingerprint.matches(line))
            {
                return Some(index);
            }
        }
        let index = anchor.index + distance;
        if index < lines.len()
            && lines
                .get(index)
                .is_some_and(|line| anchor.fingerprint.matches(line))
        {
            return Some(index);
        }
    }
    None
}

/// 近期记忆的落点：锚点之后连续 User 块的末尾（第一条 tool/assistant 之前）。
/// 遇到下一轮的真实玩家发言也停下，避免挂到新一轮上。
pub(super) fn recent_insert_index(lines: &[GameLine], anchor_index: usize) -> usize {
    let mut index = anchor_index + 1;
    while index < lines.len() {
        let line = &lines[index];
        if !matches!(line.attribute(), LineAttribute::User) {
            break;
        }
        if crate::ai_service::game_system::auto_save::is_real_dialogue(line) {
            break;
        }
        index += 1;
    }
    index
}

/// 初始化阶段没有锚点时的落点：开头连续 System 行之后。
pub(super) fn init_insert_index(lines: &[GameLine]) -> usize {
    lines
        .iter()
        .take_while(|line| matches!(line.attribute(), LineAttribute::System))
        .count()
}
