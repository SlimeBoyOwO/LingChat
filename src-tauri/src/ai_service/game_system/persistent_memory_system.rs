use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::ai_service::game_system::memory_builder::MemoryBuilder;
use crate::ai_service::llm::LlmClient;
use crate::ai_service::types::{GameLine, GameMemoryBank, GameRole, LlmMessage};

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

    llm: Arc<LlmClient>,

    memory_bank: Arc<Mutex<GameMemoryBank>>,
    is_updating: Arc<AtomicBool>,
    has_pending: Arc<AtomicBool>,

    pub enabled: bool,
    update_interval: usize,
    recent_window: usize,

    section_prompts: HashMap<String, String>,
}

impl PersistentMemorySystem {
    pub fn new(
        role_id: i32,
        initial_bank: &GameMemoryBank,
        llm: Arc<LlmClient>,
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
    pub async fn reset_from(&self, bank: &GameMemoryBank) {
        self.has_pending.store(false, Ordering::Release);
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
        let llm = self.llm.clone();
        let mb = self.memory_bank.clone();
        let prompts = self.section_prompts.clone();
        let is_updating = self.is_updating.clone();
        let has_pending = self.has_pending.clone();
        let role_id = self.role_id;
        let ai_name = self.ai_name.clone();

        tokio::spawn(async move {
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

            // 写回
            {
                let mut bank = mb.lock().await;
                bank.data.short_term = st;
                bank.data.long_term = lt;
                bank.data.user_info = ui;
                bank.data.promises = pr;
                bank.meta.last_processed_global_idx = target_idx;
                bank.meta.updated_at = now_str();
            }

            is_updating.store(false, Ordering::Release);
            has_pending.store(true, Ordering::Release);
            tracing::info!(
                "MemoryBank: role_id={} 记忆库更新完成! 指针已移动至 {}",
                role_id,
                target_idx,
            );
        });
    }

    async fn update_section(
        llm: &Arc<LlmClient>,
        prompts: &HashMap<String, String>,
        chat_text: &str,
        key: &str,
        old_content: &str,
        _ai_name: &str,
    ) -> String {
        let prompt_req = match prompts.get(key) {
            Some(p) => p,
            None => return old_content.to_string(),
        };

        let full_prompt = format!(
            "{}\n\n【旧内容】：\n{}\n\n【新增对话】：\n{}\n\n【新内容】(直接输出结果，不要废话)：",
            prompt_req, old_content, chat_text,
        );

        let messages = vec![LlmMessage::user(full_prompt)];

        match llm.complete(&messages).await {
            Ok(response) => {
                let cleaned = response.trim();
                if cleaned.is_empty() {
                    old_content.to_string()
                } else {
                    cleaned.to_string()
                }
            }
            Err(e) => {
                tracing::warn!("MemoryBank 分段压缩失败 (key={}): {}", key, e);
                old_content.to_string()
            }
        }
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
