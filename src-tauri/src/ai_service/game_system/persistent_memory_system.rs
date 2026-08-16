use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Result;

use tokio::sync::Mutex;

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::llm::{slot_snapshot, LlmClient, LlmSlot};
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

    /// 最近一次压缩失败的时间戳（unix 毫秒），0 = 无失败。用于重试冷却。
    last_failure_at_ms: Arc<AtomicU64>,
    /// 连续失败次数（诊断日志用，成功后清零）。
    fail_count: Arc<AtomicU32>,

    pub enabled: bool,
    update_interval: usize,
    recent_window: usize,

    section_prompts: HashMap<String, String>,
}

impl PersistentMemorySystem {
    pub fn new(
        role_id: i32,
        initial_bank: &GameMemoryBank,
        llm: LlmSlot,
        enabled: bool,
        update_interval: usize,
        recent_window: usize,
        display_name: &str,
    ) -> Self {
        Self {
            role_id,
            ai_name: display_name.to_string(),
            llm,
            memory_bank: Arc::new(Mutex::new(initial_bank.clone())),
            is_updating: Arc::new(AtomicBool::new(false)),
            has_pending: Arc::new(AtomicBool::new(false)),
            last_failure_at_ms: Arc::new(AtomicU64::new(0)),
            fail_count: Arc::new(AtomicU32::new(0)),
            enabled,
            update_interval,
            recent_window,
            section_prompts: init_prompts(),
        }
    }

    // ── 公开只读方法 ──

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 返回给调用方用于裁剪 line_list 的起点索引。
    pub async fn get_slice_start_index(&self) -> usize {
        let bank = self.memory_bank.lock().await;
        let idx = bank.meta.last_processed_global_idx;
        (idx - self.recent_window as i64).max(0) as usize
    }

    /// 长期记忆 / 用户画像 / 约定 文本（适合合并到 system 消息）。
    pub async fn get_system_memory_text(&self) -> String {
        let bank = self.memory_bank.lock().await;
        format!(
            "\n\n====== 记忆库 (Memory Bank) ======\n\
             【taの信息】：{}\n\
             【重要约定】：{}\n\
             【长期经历】：{}\n\
             =================================\n",
            bank.data.user_info, bank.data.promises, bank.data.long_term,
        )
    }

    /// 短期回顾文本（适合作为 user 消息前缀）。
    pub async fn get_short_term_user_text(&self) -> String {
        let bank = self.memory_bank.lock().await;
        let short = bank.data.short_term.trim();
        if short.is_empty() {
            String::new()
        } else {
            format!("【近期回顾】{}\n\n", short)
        }
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

        // 读取并校验指针
        let last_idx = {
            let bank_guard = match self.memory_bank.try_lock() {
                Ok(g) => g,
                Err(_) => return, // 后台任务正在写，跳过
            };
            let mut idx = bank_guard.meta.last_processed_global_idx;
            if idx < 0 || idx as usize > current_total {
                idx = 0;
            }
            idx as usize
        };

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
            }
            return;
        }

        tracing::info!(
            "MemoryBank: role_id={} 累积未归档可见台词 {} 条 (阈值 {})，触发自动压缩...",
            self.role_id,
            visible_count,
            self.update_interval,
        );

        self.is_updating.store(true, Ordering::Release);
        self.spawn_background_update(chat_text, target_idx);
    }

    // ── 内部方法 ──

    fn spawn_background_update(&self, chat_text: String, target_idx: i64) {
        let llm_slot = self.llm.clone();
        let mb = self.memory_bank.clone();
        let prompts = self.section_prompts.clone();
        let is_updating = self.is_updating.clone();
        let has_pending = self.has_pending.clone();
        let last_failure_at_ms = self.last_failure_at_ms.clone();
        let fail_count = self.fail_count.clone();
        let role_id = self.role_id;
        let ai_name = self.ai_name.clone();

        tokio::spawn(async move {
            // 记录一次失败并复位 is_updating。失败不推进指针 → 下轮对话重试同一批。
            let record_failure = || {
                last_failure_at_ms.store(current_time_ms(), Ordering::Release);
                fail_count.fetch_add(1, Ordering::AcqRel);
                is_updating.store(false, Ordering::Release);
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
                }
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
                    &ai_name
                ),
                Self::update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "long_term",
                    &old.long_term,
                    &ai_name
                ),
                Self::update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "user_info",
                    &old.user_info,
                    &ai_name
                ),
                Self::update_section(
                    &llm,
                    &prompts,
                    &chat_text,
                    "promises",
                    &old.promises,
                    &ai_name
                ),
            );

            // 4 段必须全部成功才写回并推进指针；任一失败则整批重试。
            let results = [st, lt, ui, pr];
            if let Some((key, err)) = results
                .iter()
                .enumerate()
                .find_map(|(i, r)| match r {
                    Err(e) => Some((["short_term", "long_term", "user_info", "promises"][i], e)),
                    Ok(_) => None,
                })
            {
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

            // 全部成功：写回 + 推进指针 + 清失败状态
            let [st, lt, ui, pr] = results;
            {
                let mut bank = mb.lock().await;
                bank.data.short_term = st.unwrap();
                bank.data.long_term = lt.unwrap();
                bank.data.user_info = ui.unwrap();
                bank.data.promises = pr.unwrap();
                bank.meta.last_processed_global_idx = target_idx;
                bank.meta.updated_at = now_str();
            }

            last_failure_at_ms.store(0, Ordering::Release);
            fail_count.store(0, Ordering::Release);
            is_updating.store(false, Ordering::Release);
            has_pending.store(true, Ordering::Release);
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
        _ai_name: &str,
    ) -> Result<String> {
        let prompt_req = match prompts.get(key) {
            Some(p) => p,
            None => return Ok(old_content.to_string()), // 配置缺失不是失败，保留旧内容
        };

        let full_prompt = format!(
            "{}\n\n【旧内容】：\n{}\n\n【新增对话】：\n{}\n\n【新内容】(直接输出结果，不要废话)：",
            prompt_req, old_content, chat_text,
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
        use crate::db::entities::line::LineAttribute;

        // 统计可见非 system 台词
        let mut visible_count: usize = 0;
        for line in lines {
            if matches!(line.attribute(), LineAttribute::System) {
                continue;
            }
            let visible = line.sender_role_id() == Some(self.role_id)
                || line.perceived_role_ids.contains(&self.role_id);
            if visible && !line.content().trim().is_empty() {
                visible_count += 1;
            }
        }

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

fn now_str() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn current_time_ms() -> u64 {
    chrono::Utc::now().timestamp_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_service::types::LineBase;

    /// 构造一条由指定角色发送的可见台词（默认 attribute 是 System，会被计数逻辑跳过）。
    fn make_line(role_id: i32, content: &str) -> GameLine {
        use crate::ai_service::types::LineAttributeExt;
        use crate::db::entities::line::LineAttribute;

        let mut base = LineBase::default();
        base.sender_role_id = Some(role_id);
        base.content = content.to_string();
        base.attribute = LineAttributeExt(LineAttribute::User);
        GameLine::from_base(base, Vec::new())
    }

    /// 构造测试用实例：role_id=1，LLM 槽位为空（后台压缩必失败）。
    fn make_sys(update_interval: usize) -> (PersistentMemorySystem, Arc<Mutex<GameMemoryBank>>) {
        let bank = GameMemoryBank::default();
        let slot: LlmSlot = Arc::new(tokio::sync::RwLock::new(None));
        let sys =
            PersistentMemorySystem::new(1, &bank, slot, true, update_interval, 2, "测试角色");
        let mb = sys.memory_bank.clone();
        (sys, mb)
    }

    /// 轮询等待后台压缩任务结束（is_updating 复位），最多让出 1000 次。
    async fn wait_for_idle(sys: &PersistentMemorySystem) {
        for _ in 0..1000 {
            if !sys.is_updating.load(Ordering::Acquire) {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("后台压缩任务未在预期时间内结束");
    }

    #[tokio::test]
    async fn below_threshold_does_not_trigger() {
        let (sys, _bank) = make_sys(10);
        let lines = vec![make_line(1, "你好呀")];
        sys.check_and_trigger_auto_update(&lines);
        // 可见台词 1 条 < 阈值 10，不应触发，也不应移动指针
        assert!(!sys.is_updating.load(Ordering::Acquire));
        assert_eq!(sys.fail_count.load(Ordering::Acquire), 0);
    }

    #[tokio::test]
    async fn reaching_threshold_triggers_background_compress() {
        let (sys, _bank) = make_sys(1);
        let lines = vec![make_line(1, "你好呀"), make_line(1, "今天天气真不错")];
        sys.check_and_trigger_auto_update(&lines);
        wait_for_idle(&sys).await;
        // 空 LLM 槽位使压缩任务走失败路径：fail_count 递增证明任务确实被触发
        assert_eq!(sys.fail_count.load(Ordering::Acquire), 1);
    }

    #[tokio::test]
    async fn failure_keeps_pointer_and_records_cooldown() {
        let (sys, bank) = make_sys(1);
        let lines = vec![make_line(1, "你好呀"), make_line(1, "今天天气真不错")];
        sys.check_and_trigger_auto_update(&lines);
        wait_for_idle(&sys).await;
        // 失败时指针不移动，供下轮对话重试同一批
        assert_eq!(bank.lock().await.meta.last_processed_global_idx, 0);
        // 冷却与失败计数均已记录
        assert_ne!(sys.last_failure_at_ms.load(Ordering::Acquire), 0);
        assert_eq!(sys.fail_count.load(Ordering::Acquire), 1);
    }

    #[tokio::test]
    async fn cooldown_blocks_retry_after_failure() {
        let (sys, _bank) = make_sys(1);
        // 模拟 1 秒前刚失败：仍在 60s 冷却期内，不应再次触发
        sys.last_failure_at_ms
            .store(current_time_ms() - 1_000, Ordering::Release);
        let lines = vec![make_line(1, "你好呀"), make_line(1, "今天天气真不错")];
        sys.check_and_trigger_auto_update(&lines);
        assert!(!sys.is_updating.load(Ordering::Acquire));
        assert_eq!(sys.fail_count.load(Ordering::Acquire), 0);
    }
}
