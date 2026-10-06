use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use anyhow::Result;
use serde::Serialize;

use tokio::sync::{Mutex, Notify};

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::llm::{LlmClient, LlmSlot, slot_snapshot};
use crate::ai_service::types::{GameLine, GameMemoryBank, GameRole, LlmMessage};

/// 压缩失败后的重试冷却时长。失败不推进指针，但为避免 LLM 故障期间每轮对话
/// 都白打 4 段压缩请求，冷却期（60s，对齐 Operit 轮询间隔）内不再触发。
const RETRY_COOLDOWN_MS: u64 = 60_000;

// ── 中文压缩提示词（与 Python PersistentMemorySystem._init_prompts 完全一致） ──

fn init_prompts() -> HashMap<String, String> {
    let base_role = concat!(
        "你是一个专业的【记忆档案管理员】。你的任务是基于【旧的记忆档案】和【新增的对话日志】，",
        "生成一份更新后的、逻辑连贯的记忆文本。\n",
        "通用规则：\n",
        "1. 视角：必须严格使用【第三人称】（例如：'（用户的名字）提到...'，'（本AI角色的名字）感到...'）。\n",
        "2. 时态：使用陈述语气，客观记录事实。\n",
        "3. 输出：直接输出更新后的内容本身，不要包含任何解释。\n",
        "4. 逻辑：如果没有新信息需要更新，请原样保留【旧的记忆档案】的内容。\n",
        "5. 内容完整性：如果【旧的记忆档案】中存在被截断或不完整的片段，请直接丢弃，不要保留或引用它们。\n",
    );

    let mut m = HashMap::new();
    m.insert(
        "short_term".to_string(),
        format!(
            "{}\n【任务目标】：生成一份【短期上下文摘要】，用于在下一次对话中承接话题。\n\
             【处理逻辑】：\n\
             1. 概括话题：他们刚才在聊什么？话题是否已经结束？\n\
             2. 捕捉氛围：当前的对话气氛如何？\n\
             3. 遗忘机制：删除旧记忆中已经过时、结束或不再相关的琐碎细节。\n\
             4. 篇幅控制：保持在 100-200 字以内。\n",
            base_role
        ),
    );
    m.insert(
        "long_term".to_string(),
        format!(
            "{}\n【任务目标】：编撰一份【角色经历编年史】，记录具有长期价值的核心事件。\n\
             【处理逻辑】：\n\
             1. 过滤噪音：忽略日常问候和闲聊。\n\
             2. 提取事件：只记录具有里程碑意义的事件。\n\
             3. 累积更新：将新发生的关键事件追加到旧档案中。\n",
            base_role
        ),
    );
    m.insert(
        "user_info".to_string(),
        format!(
            "{}\n【任务目标】：更新【taの画像】，确保 AI 了解屏幕对面的人。\n\
             【处理逻辑】：\n\
             1. 事实提取：提取用户的姓名、年龄、职业、喜好、雷点等。\n\
             2. 冲突修正：如果信息冲突（如换了工作），以【新增对话】为准。\n",
            base_role
        ),
    );
    m.insert(
        "promises".to_string(),
        format!(
            "{}\n【任务目标】：维护一份【待办与契约清单】。\n\
             【处理逻辑】：\n\
             1. 新增约定：提取对话中明确达成的承诺。\n\
             2. 状态核销：如果能够在【新增对话】中找到已完成的证据，从清单中【删除】该条目。\n",
            base_role
        ),
    );
    m
}

// ── 记忆段长度上限 ──

/// 各记忆段的长度上限（字符数）。0 = 不截断。
///
/// 截断链路：
/// - 运行时注入上下文按上限截断（仅影响本轮 LLM 可见内容，不影响存储）；
/// - 压缩时把【旧内容】按上限截断后再喂给 LLM —— LLM 只能基于截断后的内容生成
///   新记忆，因此超出上限的旧记忆片段会在本次压缩写回后被丢弃（此时会记录 warning
///   日志）。如不希望丢失，请调大对应段上限或设为 0；
/// - 压缩写回（LLM 输出的新内容）本身不截断。
#[derive(Clone, Copy, Debug)]
pub struct MemorySectionLimits {
    pub short_term: usize,
    pub long_term: usize,
    pub user_info: usize,
    pub promises: usize,
}

impl Default for MemorySectionLimits {
    fn default() -> Self {
        Self {
            short_term: 500,
            long_term: 2000,
            user_info: 800,
            promises: 800,
        }
    }
}

/// 按字符数安全截断（避免切破 UTF-8 多字节字符）。超限部分直接丢弃，无省略标记。
fn truncate_to_chars(s: &str, max_chars: usize) -> String {
    if max_chars == 0 || s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect()
}

/// 注入给 LLM 的 system 记忆块（仅 user_info / promises / long_term 三段，
/// `short_term` 走 [`render_short_term_text`] 的 user 前缀路径）。
///
/// 与 [`PersistentMemorySystem::get_system_memory_text`] 同源：抽成纯函数是为了让
/// 调试快照能在**同一把锁**里拿到"存储"与"注入"两个视图，避免撕裂值。
fn render_system_memory_text(bank: &GameMemoryBank, limits: MemorySectionLimits) -> String {
    format!(
        "\n\n====== 记忆库 (Memory Bank) ======\n\
         【taの信息】：{}\n\
         【重要约定】：{}\n\
         【长期经历】：{}\n\
         =================================\n",
        truncate_to_chars(&bank.data.user_info, limits.user_info),
        truncate_to_chars(&bank.data.promises, limits.promises),
        truncate_to_chars(&bank.data.long_term, limits.long_term),
    )
}

/// 注入给 LLM 的短期回顾前缀；为空或仍是占位符时返回空串（= 完全不注入）。
fn render_short_term_text(bank: &GameMemoryBank, limits: MemorySectionLimits) -> String {
    let short = truncate_to_chars(bank.data.short_term.trim(), limits.short_term);
    if short.is_empty() || short == "暂无近期对话摘要。" {
        String::new()
    } else {
        format!("【近期回顾】{}\n\n", short)
    }
}

// ── 结构体 ──

/// 面向 0.4.0 新架构的"永久记忆（MemoryBank）+ 自动压缩"实现（运行时缓存版）。
///
/// - 不直接做 DB 读写：仅更新内部 `Arc<Mutex<GameMemoryBank>>`
/// - 当累计"该角色可见台词"达到阈值时，触发后台 LLM 总结
/// - 对 LLM 上下文：通过 `get_slice_start_index()` 控制裁剪窗口
///
/// 线程安全设计：
/// - `memory_bank` 与 `is_updating` / `has_pending` 均通过 Arc 共享，
///   使得 `tokio::spawn` 的后台任务可以安全写入压缩结果。
/// - `sync_to_role()` 在下次 `sync_memories()` 时通过 try_lock 非阻塞同步。
pub struct PersistentMemorySystem {
    #[allow(dead_code)]
    role_id: i32,
    ai_name: String,

    /// LLM 槽位（支持运行时热切换）。
    llm: LlmSlot,

    memory_bank: Arc<Mutex<GameMemoryBank>>,
    is_updating: Arc<AtomicBool>,
    has_pending: Arc<AtomicBool>,
    /// 每次历史刷新都会递增；后台任务提交前必须仍匹配，避免撤回/读档后写入旧摘要。
    history_revision: Arc<AtomicU64>,
    /// 把历史失效与后台成功/失败提交放进同一同步边界，封闭最终 revision 检查的 TOCTOU。
    commit_gate: Arc<std::sync::Mutex<()>>,
    /// 后台压缩结束时的唤醒源：`compress_if_needed` 靠它等到本轮压缩落回来。
    idle_notify: Arc<Notify>,

    /// 最近一次压缩失败的时间戳（unix 毫秒），0 = 无失败。用于重试冷却。
    last_failure_at_ms: Arc<AtomicU64>,
    /// 连续失败次数（诊断日志用，成功后清零）。
    fail_count: Arc<AtomicU32>,

    pub enabled: bool,
    update_interval: usize,
    recent_window: usize,
    /// 各记忆段注入/压缩时的长度上限（运行时注入截断 + 压缩喂入截断；
    /// 压缩写回不截断，但超限旧片段会在压缩时被丢弃）。
    section_limits: MemorySectionLimits,

    section_prompts: HashMap<String, String>,
}

/// 无论后台任务成功、显式失败还是 panic 展开，都解除 updating 锁。
struct UpdatingGuard(Arc<AtomicBool>, Arc<Notify>);

impl Drop for UpdatingGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
        // 唤醒可能正卡在 `wait_until_idle` 的调用方。无等待者时 notify_one 会
        // 存下一个 permit，兜住「判空后、await 前刚好完成」的漏唤醒竞态。
        self.1.notify_one();
    }
}

fn record_failure_if_current(
    commit_gate: &std::sync::Mutex<()>,
    history_revision: &AtomicU64,
    expected_revision: u64,
    last_failure_at_ms: &AtomicU64,
    fail_count: &AtomicU32,
) -> bool {
    let _gate = commit_gate
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if history_revision.load(Ordering::Acquire) != expected_revision {
        return false;
    }
    last_failure_at_ms.store(current_time_ms(), Ordering::Release);
    fail_count.fetch_add(1, Ordering::AcqRel);
    true
}

#[allow(clippy::too_many_arguments)]
fn commit_update_if_current(
    commit_gate: &std::sync::Mutex<()>,
    history_revision: &AtomicU64,
    expected_revision: u64,
    bank: &mut GameMemoryBank,
    sections: [String; 4],
    target_idx: i64,
    last_failure_at_ms: &AtomicU64,
    fail_count: &AtomicU32,
    has_pending: &AtomicBool,
) -> bool {
    let _gate = commit_gate
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if history_revision.load(Ordering::Acquire) != expected_revision {
        return false;
    }
    let [short_term, long_term, user_info, promises] = sections;
    bank.data.short_term = short_term;
    bank.data.long_term = long_term;
    bank.data.user_info = user_info;
    bank.data.promises = promises;
    bank.meta.last_processed_global_idx = target_idx;
    bank.meta.updated_at = now_str();
    last_failure_at_ms.store(0, Ordering::Release);
    fail_count.store(0, Ordering::Release);
    has_pending.store(true, Ordering::Release);
    true
}

impl PersistentMemorySystem {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role_id: i32,
        initial_bank: &GameMemoryBank,
        llm: LlmSlot,
        enabled: bool,
        update_interval: usize,
        recent_window: usize,
        limits: MemorySectionLimits,
        display_name: &str,
    ) -> Self {
        Self {
            role_id,
            ai_name: display_name.to_string(),
            llm,
            memory_bank: Arc::new(Mutex::new(initial_bank.clone())),
            is_updating: Arc::new(AtomicBool::new(false)),
            has_pending: Arc::new(AtomicBool::new(false)),
            history_revision: Arc::new(AtomicU64::new(0)),
            commit_gate: Arc::new(std::sync::Mutex::new(())),
            idle_notify: Arc::new(Notify::new()),
            last_failure_at_ms: Arc::new(AtomicU64::new(0)),
            fail_count: Arc::new(AtomicU32::new(0)),
            enabled,
            update_interval,
            recent_window,
            section_limits: limits,
            section_prompts: init_prompts(),
        }
    }

    // ── 公开只读方法 ──

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 在台词历史发生追加、撤回、清空或读档时使进行中的后台摘要立即过期。
    pub fn invalidate_history(&self) {
        let _gate = self
            .commit_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.history_revision.fetch_add(1, Ordering::AcqRel);
    }

    /// 返回给调用方用于裁剪 line_list 的起点索引。
    ///
    /// `recent_window` 的单位与触发阈值一致，都是“该角色可见、非 system 的台词”，
    /// 而不是全局原始行数；0 表示压缩点之前一条也不保留。
    pub async fn get_slice_start_index(&self, all_lines: &[GameLine]) -> usize {
        let bank = self.memory_bank.lock().await;
        let processed = bank
            .meta
            .last_processed_global_idx
            .max(0)
            .min(all_lines.len() as i64) as usize;
        drop(bank);

        if processed == 0 || self.recent_window == 0 {
            return processed;
        }

        let mut start = processed;
        let mut remaining = self.recent_window;
        while start > 0 {
            start -= 1;
            if line_visible_to_role(&all_lines[start], self.role_id) {
                remaining -= 1;
                if remaining == 0 {
                    break;
                }
            }
        }
        start
    }

    /// 把压缩指针回拨到 `idx`（仅当当前指针在其之后）。用于编辑台词历史后让被
    /// 编辑区间重新进入渲染窗口、并在下次压缩时重新摘要。
    ///
    /// 回拨会置 `has_pending`，使新指针经 `sync_to_role` 同步进 `role.memory_bank`，
    /// 进而被 `persist_memory_banks_to_db` 落库；否则存盘写回的仍是旧指针。
    pub async fn rewind_pointer(&self, idx: usize) {
        let mut bank = self.memory_bank.lock().await;
        if bank.meta.last_processed_global_idx.max(0) as usize > idx {
            bank.meta.last_processed_global_idx = idx as i64;
            bank.meta.updated_at = now_str();
            self.has_pending.store(true, Ordering::Release);
        }
    }

    /// 长期记忆 / 用户画像 / 约定 文本（适合合并到 system 消息）。
    /// 各段按 `section_limits` 截断后注入（存储不截断，仅运行时视图截断）。
    pub async fn get_system_memory_text(&self) -> String {
        let bank = self.memory_bank.lock().await;
        render_system_memory_text(&bank, self.section_limits)
    }

    /// 短期回顾文本（适合作为 user 消息前缀）。
    /// 按 `section_limits.short_term` 截断后注入。
    pub async fn get_short_term_user_text(&self) -> String {
        let bank = self.memory_bank.lock().await;
        render_short_term_text(&bank, self.section_limits)
    }

    // ── 同步写回 ──

    /// 非阻塞：若后台任务已完成且未同步，将压缩结果写回 `GameRole`。
    pub fn sync_to_role(&self, role: &mut GameRole) {
        if !self.has_pending.load(Ordering::Acquire) {
            return;
        }
        if let Ok(bank) = self.memory_bank.try_lock() {
            role.memory_bank = bank.clone();
            self.has_pending.store(false, Ordering::Release);
        }
    }

    /// 从 DB 加载后重置内部缓存（丢弃任何待处理的过期更新）。
    /// 同时清失败状态：指针仍停在 DB 中的旧位置，重试从新会话重新计时。
    pub async fn reset_from(&self, bank: &GameMemoryBank) {
        self.invalidate_history();
        self.has_pending.store(false, Ordering::Release);
        self.last_failure_at_ms.store(0, Ordering::Release);
        self.fail_count.store(0, Ordering::Release);
        let mut mb = self.memory_bank.lock().await;
        *mb = bank.clone();
    }

    // ── 触发检查（主线程调用） ──

    /// 检查是否达到阈值，若是则触发后台压缩。
    /// 对标 Python `PersistentMemorySystem.check_and_trigger_auto_update`。
    pub fn check_and_trigger_auto_update(&self, all_lines: &[GameLine]) {
        if !self.enabled {
            return;
        }
        let history_revision = self.history_revision.load(Ordering::Acquire);
        if self.is_updating.load(Ordering::Acquire) {
            return;
        }

        // 失败重试冷却：压缩失败后不推进指针（下轮对话仍会重试同一批），
        // 但冷却期内不再触发，避免 LLM 故障时每轮对话都白打 4 段压缩请求。
        let last_fail = self.last_failure_at_ms.load(Ordering::Acquire);
        // saturating_sub：系统时钟回拨（NTP 校准等）时避免 u64 下溢导致 Debug panic /
        // Release 回绕使冷却失效。
        if last_fail != 0 && current_time_ms().saturating_sub(last_fail) < RETRY_COOLDOWN_MS {
            return;
        }

        let current_total = all_lines.len();

        // 读取并校验指针。越界（清空对话/读档后 line_list 变短）时**写回**重置，
        // 否则指针残留旧值，get_slice_start_index 会一直返回过期大索引，
        // 导致上下文窗口无限膨胀且每轮都从 index 0 重建整段上下文。
        let (last_idx, pointer_corrected) = {
            let mut bank_guard = match self.memory_bank.try_lock() {
                Ok(g) => g,
                Err(_) => return, // 后台任务正在写，跳过
            };
            let idx = bank_guard.meta.last_processed_global_idx;
            if idx < 0 || idx as usize > current_total {
                bank_guard.meta.last_processed_global_idx = 0;
                bank_guard.meta.updated_at = now_str();
                (0, true)
            } else {
                (idx as usize, false)
            }
        };
        if pointer_corrected {
            self.has_pending.store(true, Ordering::Release);
        }

        let new_lines = &all_lines[last_idx..current_total];
        let (chat_text, visible_count) = self.build_chat_text_and_count(new_lines);
        let target_idx = current_total as i64;

        if visible_count < self.update_interval {
            return;
        }

        if chat_text.trim().is_empty() {
            // 区间对该角色完全不可见，直接移动指针避免无限触发
            if let Ok(mut bank) = self.memory_bank.try_lock() {
                bank.meta.last_processed_global_idx = target_idx;
                bank.meta.updated_at = now_str();
                self.has_pending.store(true, Ordering::Release);
            }
            return;
        }

        tracing::info!(
            "MemoryBank: role_id={} 累积未归档可见台词 {} 条 (阈值 {})，触发自动压缩...",
            self.role_id,
            visible_count,
            self.update_interval,
        );

        if self
            .is_updating
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        self.spawn_background_update(chat_text, target_idx, history_revision);
    }

    /// 达到压缩阈值就同步跑一次压缩并**等它完成**；未达阈值 / 冷却中 / 区间对该
    /// 角色不可见时立即返回。返回是否实际触发了压缩。
    ///
    /// 与后台自动压缩共用同一套判定（`check_and_trigger_auto_update`），区别只是
    /// 把「spawn 完就不管」换成「等回来」。若调用时已有压缩在跑，会先等它结束再
    /// 重新判定，确保返回时该角色没有「待压缩」的积压。
    ///
    /// 会 `await` 若干次 LLM 调用，**不要在持有 `game_status` 锁时调用**。
    pub async fn compress_if_needed(&self, all_lines: &[GameLine]) -> bool {
        if !self.enabled {
            return false;
        }
        let mut triggered = false;
        loop {
            // 已有压缩在跑就先等它落回来，避免叠加判定。
            self.wait_until_idle().await;
            self.check_and_trigger_auto_update(all_lines);
            if !self.is_updating.load(Ordering::Acquire) {
                return triggered;
            }
            // 触发成功 → 回到循环开头等它完成，再判定是否还有新积压。
            triggered = true;
        }
    }

    /// 等到当前这轮后台压缩结束；没有在跑时立即返回。
    async fn wait_until_idle(&self) {
        while self.is_updating.load(Ordering::Acquire) {
            self.idle_notify.notified().await;
        }
    }

    // ── 内部方法 ──

    fn spawn_background_update(
        &self,
        chat_text: String,
        target_idx: i64,
        expected_history_revision: u64,
    ) {
        let llm_slot = self.llm.clone();
        let mb = self.memory_bank.clone();
        let prompts = self.section_prompts.clone();
        let is_updating = self.is_updating.clone();
        let has_pending = self.has_pending.clone();
        let history_revision = self.history_revision.clone();
        let commit_gate = self.commit_gate.clone();
        let last_failure_at_ms = self.last_failure_at_ms.clone();
        let fail_count = self.fail_count.clone();
        let idle_notify = self.idle_notify.clone();
        let role_id = self.role_id;
        let ai_name = self.ai_name.clone();
        let limits = self.section_limits;

        tokio::spawn(async move {
            let _updating_guard = UpdatingGuard(is_updating.clone(), idle_notify);
            // 仅当前历史版本的失败才进入冷却；过期任务不能污染新会话的重试状态。
            let record_failure = || {
                if !record_failure_if_current(
                    &commit_gate,
                    &history_revision,
                    expected_history_revision,
                    &last_failure_at_ms,
                    &fail_count,
                ) {
                    tracing::info!(
                        "MemoryBank: role_id={} 过期压缩请求失败，忽略其冷却状态",
                        role_id
                    );
                }
            };

            // 从槽位读取当前 LLM 客户端快照（支持热切换后使用新模型）
            let llm = match slot_snapshot(&llm_slot).await {
                Some(client) => client,
                None => {
                    tracing::warn!(
                        "MemoryBank: role_id={} LLM 槽位为空，跳过本次更新（第 {} 次失败）",
                        role_id,
                        fail_count.load(Ordering::Acquire) + 1
                    );
                    record_failure();
                    return;
                },
            };

            // 读取旧内容
            let old_bank = mb.lock().await.clone();
            let old = &old_bank.data;

            let (st, lt, ui, pr) = tokio::join!(
                Self::update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "short_term",
                    &old.short_term,
                    limits.short_term,
                    &ai_name
                ),
                Self::update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "long_term",
                    &old.long_term,
                    limits.long_term,
                    &ai_name
                ),
                Self::update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "user_info",
                    &old.user_info,
                    limits.user_info,
                    &ai_name
                ),
                Self::update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "promises",
                    &old.promises,
                    limits.promises,
                    &ai_name
                ),
            );

            // 4 段必须全部成功才写回并推进指针；任一失败则整批重试。
            let results = [st, lt, ui, pr];
            if let Some((key, err)) = results.iter().enumerate().find_map(|(i, r)| match r {
                Err(e) => Some((["short_term", "long_term", "user_info", "promises"][i], e)),
                Ok(_) => None,
            }) {
                let count = fail_count.load(Ordering::Acquire) + 1;
                tracing::warn!(
                    "MemoryBank: role_id={} 分段压缩失败 (key={}): {}（第 {} 次失败，指针不移动，冷却后重试）",
                    role_id,
                    key,
                    err,
                    count,
                );
                record_failure();
                return;
            }

            // 压缩期间若历史被追加、撤回、清空或读档，本批输入已经过期，绝不能写回。
            if history_revision.load(Ordering::Acquire) != expected_history_revision {
                tracing::info!(
                    "MemoryBank: role_id={} 历史版本已变化，丢弃过期压缩结果（指针不移动）",
                    role_id
                );
                return;
            }

            // 全部成功：在同一个提交门内复核版本、写回、推进指针并清失败状态。
            let [st, lt, ui, pr] = results;
            let sections = [st.unwrap(), lt.unwrap(), ui.unwrap(), pr.unwrap()];
            let mut bank = mb.lock().await;
            if !commit_update_if_current(
                &commit_gate,
                &history_revision,
                expected_history_revision,
                &mut bank,
                sections,
                target_idx,
                &last_failure_at_ms,
                &fail_count,
                &has_pending,
            ) {
                tracing::info!(
                    "MemoryBank: role_id={} 提交前历史版本已变化，丢弃过期压缩结果",
                    role_id
                );
                return;
            }
            drop(bank);
            tracing::info!(
                "MemoryBank: role_id={} 记忆库更新完成! 指针已移动至 {}",
                role_id,
                target_idx,
            );
        });
    }

    /// 返回 `Ok(新内容)` 或传播 LLM 错误。调用方负责失败重试（不推进指针）。
    async fn update_section(
        llm: &Arc<LlmClient>,
        prompts: &HashMap<String, String>,
        chat_text: &str,
        key: &str,
        old_content: &str,
        max_chars: usize,
        _ai_name: &str,
    ) -> Result<String> {
        let prompt_req = match prompts.get(key) {
            Some(p) => p,
            None => return Ok(old_content.to_string()), // 配置缺失不是失败，保留旧内容
        };

        // 喂给压缩 LLM 前按上限截断旧内容。LLM 只能基于截断后的内容生成新记忆，
        // 因此超出上限的旧记忆片段会在本次压缩写回后被丢弃（写回本身不截断）。
        let original_count = old_content.chars().count();
        let exceeds_limit = max_chars != 0 && original_count > max_chars;
        let old = truncate_to_chars(old_content, max_chars);
        if exceeds_limit {
            tracing::warn!(
                "MemoryBank: 记忆段 '{}' 旧内容超长 ({} 字符 > 上限 {} 字符)，超限尾部将被本次压缩丢弃；如不希望丢失请调大上限或设为 0",
                key,
                original_count,
                max_chars
            );
        }

        let full_prompt = format!(
            "{}\n\n【旧内容】：\n{}\n\n【新增对话】：\n{}\n\n【新内容】(直接输出结果，不要废话)：",
            prompt_req, old, chat_text,
        );

        let messages = vec![LlmMessage::user(full_prompt)];

        let response = llm.complete(&messages).await?;
        let cleaned = response.trim();
        if cleaned.is_empty() {
            // 空响应视为失败：部分 provider 故障时可能返回空串。若按成功处理，
            // 会把空内容写回并推进指针，静默丢弃该批对话，违背重试语义。
            return Err(anyhow::anyhow!("LLM 返回空内容"));
        }
        Ok(cleaned.to_string())
    }

    /// 构建用于压缩的对话文本 + 该角色可见台词计数。
    ///
    /// 对标 Python `PersistentMemorySystem._build_chat_text_and_count`：
    /// 1. 统计非 system 且该角色可见的台词数（visible_count）
    /// 2. 用 MemoryBuilder 构建该角色视角的 LLM 消息，转为纯文本
    fn build_chat_text_and_count(&self, lines: &[GameLine]) -> (String, usize) {
        // 统计可见非 system 台词
        let visible_count = lines
            .iter()
            .filter(|line| line_visible_to_role(line, self.role_id))
            .count();

        if visible_count == 0 {
            return (String::new(), 0);
        }

        // 用 MemoryBuilder 构建角色视角上下文
        let builder = MemoryBuilder::new(self.role_id);
        let built = builder.build(lines);

        let mut chunks: Vec<String> = Vec::new();
        for msg in &built {
            let c = msg.content.trim();
            if c.is_empty() {
                continue;
            }
            match msg.role.as_str() {
                "system" => continue,
                "assistant" => chunks.push(format!("{}: {}", self.ai_name, c)),
                "user" => chunks.push(format!("User: {}", c)),
                other => chunks.push(format!("{}: {}", other, c)),
            }
        }

        let chat_text = chunks.join("\n");
        if !chat_text.is_empty() {
            (format!("{}\n", chat_text), visible_count)
        } else {
            (chat_text, visible_count)
        }
    }
}

// ── 调试快照（只读）──

/// 单个记忆段的「存储字符数 / 注入上限」对照，用于解释注入时被截断了多少。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySectionStat {
    /// `short_term` / `long_term` / `user_info` / `promises`。
    pub key: &'static str,
    /// 存储真源里的字符数（`GameMemoryBank` 不截断）。
    pub stored_chars: usize,
    /// 该段的注入上限；`0` = 不截断。
    pub limit: usize,
    /// 注入时是否会被截断（`limit != 0 && stored_chars > limit`）。
    pub truncated: bool,
}

/// 永久记忆运行时的只读快照，供前端调试页展示。
///
/// **这里是纯读路径**：不写回 bank、不置 `has_pending`、不触发压缩。
/// 特别地，指针越界时只回报 `pointer_out_of_range`，**不顺手重置**——
/// 重置是 `check_and_trigger_auto_update` 的写路径，它会经 `sync_to_role()`
/// 传染到 `GameRole.memory_bank`，并最终由 `persist_memory_banks_to_db()`
/// 落库；只读入口绝不能触发持久化写入。
/// （计数口径与阈值判定一致：见 `check_and_trigger_auto_update` 里的
/// `all_lines[last_idx..current_total]`。）
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySystemSnapshot {
    /// 运行时真源，**不是** `GameRole.memory_bank` 那个滞后副本
    /// （副本只在 `sync_to_role()` 且 `has_pending == true` 时更新）。
    pub bank: GameMemoryBank,
    pub sections: Vec<MemorySectionStat>,
    /// LLM 实际收到的 system 记忆块（已按上限截断；注意 `short_term` **不在**这一块里）。
    pub injected_system_text: String,
    /// LLM 实际收到的短期回顾前缀；空串表示完全不注入（为空或仍是占位符）。
    pub injected_short_term_text: String,
    pub enabled: bool,
    /// 构造时烘焙的角色名；压缩提示词里用的是它，角色改名后不会更新。
    pub ai_name: String,
    /// 以下是**运行时生效值**（构造时定死，改配置需重启）。
    pub update_interval: usize,
    pub recent_window: usize,
    /// bank 里记录的原始指针（未做钳制）。
    pub pointer_raw: i64,
    /// 实际用于计数/切片的指针（钳制到 `[0, line_count]`）。
    pub pointer_effective: usize,
    /// 指针越界（撤回/读档/清空后历史变短）——下次触发时会被重置为 0。
    pub pointer_out_of_range: bool,
    /// 指针之后、该角色可见的非 system 台词数（= 触发进度）。
    pub accumulated_visible_count: usize,
    /// 当前台词历史总行数（用于解释指针）。
    pub line_count: usize,
    pub is_updating: bool,
    /// 有未同步的后台结果。注意它**不等于**"压缩刚完成"：
    /// 指针越界修正与空区间推进也会置位。
    pub has_pending: bool,
    pub fail_count: u32,
    pub history_revision: u64,
    /// 失败冷却剩余毫秒；`0` = 不在冷却中。
    pub cooldown_remaining_ms: u64,
}

impl PersistentMemorySystem {
    /// 取一份只读快照。
    ///
    /// 内部**只锁一次** bank：分成两次取值会产生"指针是 A 时刻、计数是 B 时刻"
    /// 的撕裂结果，对调试页是致命的。
    pub async fn debug_snapshot(&self, all_lines: &[GameLine]) -> MemorySystemSnapshot {
        let bank = self.memory_bank.lock().await.clone();

        let pointer_raw = bank.meta.last_processed_global_idx;
        let pointer_out_of_range = pointer_raw < 0 || pointer_raw as usize > all_lines.len();
        let pointer_effective = pointer_raw.clamp(0, all_lines.len() as i64) as usize;
        let accumulated_visible_count = all_lines[pointer_effective..]
            .iter()
            .filter(|line| line_visible_to_role(line, self.role_id))
            .count();

        let limits = self.section_limits;
        let sections = [
            ("short_term", &bank.data.short_term, limits.short_term),
            ("long_term", &bank.data.long_term, limits.long_term),
            ("user_info", &bank.data.user_info, limits.user_info),
            ("promises", &bank.data.promises, limits.promises),
        ]
        .into_iter()
        .map(|(key, text, limit)| {
            let stored_chars = text.chars().count();
            MemorySectionStat {
                key,
                stored_chars,
                limit,
                truncated: limit != 0 && stored_chars > limit,
            }
        })
        .collect();

        let last_fail = self.last_failure_at_ms.load(Ordering::Acquire);
        let cooldown_remaining_ms = if last_fail == 0 {
            0
        } else {
            last_fail
                .saturating_add(RETRY_COOLDOWN_MS)
                .saturating_sub(current_time_ms())
        };

        MemorySystemSnapshot {
            injected_system_text: render_system_memory_text(&bank, limits),
            injected_short_term_text: render_short_term_text(&bank, limits),
            bank,
            sections,
            enabled: self.enabled,
            ai_name: self.ai_name.clone(),
            update_interval: self.update_interval,
            recent_window: self.recent_window,
            pointer_raw,
            pointer_effective,
            pointer_out_of_range,
            accumulated_visible_count,
            line_count: all_lines.len(),
            is_updating: self.is_updating.load(Ordering::Acquire),
            has_pending: self.has_pending.load(Ordering::Acquire),
            fail_count: self.fail_count.load(Ordering::Acquire),
            history_revision: self.history_revision.load(Ordering::Acquire),
            cooldown_remaining_ms,
        }
    }
}

fn line_visible_to_role(line: &GameLine, role_id: i32) -> bool {
    use crate::db::entities::line::LineAttribute;

    !matches!(line.attribute(), LineAttribute::System)
        && !line.content().trim().is_empty()
        && (line.sender_role_id() == Some(role_id) || line.perceived_role_ids.contains(&role_id))
}

fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn current_time_ms() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}
