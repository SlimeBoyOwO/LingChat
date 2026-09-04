//! 上下文窗口管理：token 用量估算 + kimi-cli 式总结压缩。
//!
//! 与永久记忆（MemoryBank，按角色长期记忆）相互独立，两者可叠加：
//! - 本模块按存档把「第 cutoff 条之前的台词」压缩成一段交接笔记式摘要，
//!   构建 LLM 上下文时旧台词被摘要替代、最近 `KEEP_RECENT_LINES` 条保留原文；
//! - 用量显示用「API 实测锚点 + 本地估算增量」混合口径（参考 kimi-code
//!   tokenCounting）：每轮生成结束 producer 把 provider 上报的 prompt_tokens
//!   写入 `GameStatus.last_prompt_tokens` 锚点，之后新增台词本地估算补上。
//!
//! 摘要持久化在 `context_summary` 表（按 save_id 一行），重启/读档后依然生效。

use anyhow::{Result, anyhow};
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::AppState;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::llm::slot_snapshot;
use crate::ai_service::types::{GameLine, LlmMessage};
use crate::db::entities::context_summary;
use crate::db::entities::line::LineAttribute;

/// 模型未配置上下文窗口时的默认假设（128k）
pub const DEFAULT_CONTEXT_WINDOW: u32 = 128_000;
/// 自动压缩触发阈值：估算用量 ≥ 窗口 × 0.85（kimi-code triggerRatio）
pub const AUTO_COMPACT_RATIO: f64 = 0.85;
/// 压缩时保留的最近台词条数（原文保留，不进摘要）
pub const KEEP_RECENT_LINES: usize = 20;
/// 台词总数低于该值时不值得压缩
pub const MIN_LINES_TO_COMPACT: usize = 40;
/// 喂给压缩 LLM 的旧对话文本字符上限（超出时掐头去尾保留两端）
const COMPACT_INPUT_MAX_CHARS: usize = 24_000;
/// 没有实测锚点时，system 提示/人设等固定开销的粗估值
const SYSTEM_PROMPT_ESTIMATE: u32 = 1_500;

/// 压缩 prompt：交接笔记式（kimi-code compaction-instruction 的角色扮演语境改编）
const COMPACT_PROMPT: &str = r#"你是一个对话存档员。下面的【旧对话】是一个角色扮演聊天中即将被压缩丢弃的部分。请把它们写成一段 500 字以内的「前情摘要」，供后续对话直接续接使用。

必须保留：
- 剧情事实：发生了什么、谁在场上、场景与时间线的关键变化
- 关系状态：角色与玩家的关系进展、重要的情绪转折
- 约定与偏好：玩家的要求/约定、角色的承诺、用户明确表达过的喜好
- 未解伏笔：尚未收回的话题、承诺过但还没做的事

要求：用中文、第三人称客观叙述；直接输出摘要正文，不要解释、不要列表标题以外的废话；丢失细节可以接受，但不得编造旧对话里没有的内容。"#;

// ============================================================
// token 估算
// ============================================================

/// 本地 token 估算：ASCII 约 4 字符 1 token，非 ASCII 每字符 1 token，
/// 每条台词另加 4 token 的消息结构开销（与 kimi-code estimateTokens 同口径）。
pub fn estimate_text_tokens(text: &str) -> u32 {
    let mut ascii = 0u32;
    let mut non_ascii = 0u32;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii += 1;
        } else {
            non_ascii += 1;
        }
    }
    ascii.div_ceil(4) + non_ascii
}

pub fn estimate_lines_tokens(lines: &[GameLine]) -> u32 {
    lines
        .iter()
        .map(|line| estimate_text_tokens(line.content()) + 4)
        .sum()
}

// ============================================================
// 用量读取
// ============================================================

/// 当前生效的压缩摘要（cutoff 未越界才有效）。
pub fn effective_summary(status: &GameStatus) -> Option<(&str, usize)> {
    match (&status.context_summary, status.context_summary_cutoff) {
        (Some(summary), cutoff) if cutoff > 0 && cutoff <= status.line_list.len() => {
            Some((summary.as_str(), cutoff))
        },
        _ => None,
    }
}

/// 当前上下文用量估算（tokens）。
///
/// - 摘要生效时：锚点已失效（上下文结构变了），改为估算 摘要 + 保留段 + 固定开销
/// - 有实测锚点且锚点未越界：锚点 prompt_tokens + 锚点后新增台词的估算
/// - 否则：全量台词估算 + 固定开销
pub fn current_usage_tokens(status: &GameStatus) -> u32 {
    let lines = &status.line_list;
    if let Some((summary, cutoff)) = effective_summary(status) {
        return SYSTEM_PROMPT_ESTIMATE
            + estimate_text_tokens(summary)
            + estimate_lines_tokens(&lines[cutoff..]);
    }
    if let Some(anchor) = status.last_prompt_tokens {
        if status.last_usage_line_count <= lines.len() {
            return anchor + estimate_lines_tokens(&lines[status.last_usage_line_count..]);
        }
    }
    SYSTEM_PROMPT_ESTIMATE + estimate_lines_tokens(lines)
}

/// 当前对话模型的上下文窗口大小（未配置回退 128k）。
pub async fn current_context_window(app: &AppHandle) -> u32 {
    crate::ai_service::llm::provider_config::resolve_chat_provider(app)
        .and_then(|p| p.context_window)
        .unwrap_or(DEFAULT_CONTEXT_WINDOW)
}

// ============================================================
// 压缩
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactOutcome {
    /// 本次压缩掉的台词条数；0 表示无需压缩
    pub compacted_lines: usize,
    /// 压缩后保留的原文条数
    pub kept_lines: usize,
    /// 压缩后估算用量
    pub used_tokens_after: u32,
    pub message: String,
}

/// 找切点：保留最近 KEEP_RECENT_LINES 条，且切点落在 user 台词边界上
///（不把某个角色的回复拦腰切断）。
fn find_cutoff(lines: &[GameLine]) -> usize {
    if lines.len() <= KEEP_RECENT_LINES {
        return 0;
    }
    let mut cut = lines.len() - KEEP_RECENT_LINES;
    while cut > 0 && !matches!(lines[cut].attribute(), LineAttribute::User) {
        cut -= 1;
    }
    cut
}

/// 把台词渲染成纯文本供压缩 LLM 阅读（发言者: 内容）。
fn lines_to_plain_text(lines: &[GameLine]) -> String {
    let mut out = String::new();
    for line in lines {
        let speaker = line
            .base
            .display_name
            .as_deref()
            .unwrap_or_else(|| match line.attribute() {
                LineAttribute::User => "玩家",
                LineAttribute::Assistant => "角色",
                LineAttribute::System => "系统",
                LineAttribute::Tool => "工具",
            });
        out.push_str(speaker);
        out.push('：');
        out.push_str(line.content());
        out.push('\n');
    }
    out
}

/// 掐头去尾：保留前 60% 与后 40%，中间标注省略。
fn truncate_middle(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        return text.to_string();
    }
    let head = max_chars * 3 / 5;
    let tail = max_chars - head;
    let head_str: String = text.chars().take(head).collect();
    let tail_str: String = text.chars().skip(total - tail).collect();
    format!(
        "{}\n\n……（此处省略约 {} 字）……\n\n{}",
        head_str,
        total - head - tail,
        tail_str
    )
}

/// 执行压缩：生成摘要、写库、更新 GameStatus。`force=false` 时行数太少直接跳过。
pub async fn compact_now(app: &AppHandle, force: bool) -> Result<CompactOutcome> {
    let state = app.state::<AppState>();
    let svc = state.ai_service.lock().await;
    let gs_arc = svc.game_status.clone();
    drop(svc);

    // 快照当前台词与存档
    let (lines, save_id) = {
        let gs = gs_arc.lock().await;
        (gs.line_list.clone(), gs.active_save_id)
    };

    if lines.len() < MIN_LINES_TO_COMPACT {
        return Ok(CompactOutcome {
            compacted_lines: 0,
            kept_lines: lines.len(),
            used_tokens_after: current_usage_tokens(&*gs_arc.lock().await),
            message: format!("当前只有 {} 条对话，无需压缩", lines.len()),
        });
    }

    let cut = find_cutoff(&lines);
    if cut == 0 {
        return Ok(CompactOutcome {
            compacted_lines: 0,
            kept_lines: lines.len(),
            used_tokens_after: current_usage_tokens(&*gs_arc.lock().await),
            message: "可压缩区间为空，无需压缩".to_string(),
        });
    }
    if !force {
        let window = current_context_window(app).await;
        let used = current_usage_tokens(&*gs_arc.lock().await);
        if (used as f64) < window as f64 * AUTO_COMPACT_RATIO {
            return Ok(CompactOutcome {
                compacted_lines: 0,
                kept_lines: lines.len(),
                used_tokens_after: used,
                message: "用量未到阈值，无需压缩".to_string(),
            });
        }
    }

    let llm = slot_snapshot(&state.chat.llm)
        .await
        .ok_or_else(|| anyhow!("LLM 未配置，无法压缩"))?;

    let old_text = truncate_middle(&lines_to_plain_text(&lines[..cut]), COMPACT_INPUT_MAX_CHARS);
    // 已有旧摘要时连同旧摘要一起续写，避免多轮压缩丢失远期内容
    let prior_summary = {
        let gs = gs_arc.lock().await;
        effective_summary(&gs).map(|(s, _)| s.to_string())
    };
    let prompt = match prior_summary {
        Some(prev) => format!(
            "{}\n\n【上次摘要】：\n{}\n\n【旧对话】：\n{}\n\n【前情摘要】（直接输出结果）：",
            COMPACT_PROMPT, prev, old_text
        ),
        None => format!(
            "{}\n\n【旧对话】：\n{}\n\n【前情摘要】（直接输出结果）：",
            COMPACT_PROMPT, old_text
        ),
    };

    let response = llm.complete(&[LlmMessage::user(prompt)]).await?;
    let summary = response.trim().to_string();
    if summary.is_empty() {
        return Err(anyhow!("压缩 LLM 返回空内容，已保留原状"));
    }

    // 写回内存态 + 持久化；实测锚点失效（上下文结构已变）
    {
        let mut gs = gs_arc.lock().await;
        // 压缩期间若对话有新增台词，cutoff 语义仍成立（cut 是按快照算的条数）
        gs.context_summary = Some(summary.clone());
        gs.context_summary_cutoff = cut;
        gs.last_prompt_tokens = None;
        gs.last_usage_line_count = 0;
    }

    if let Some(save_id) = save_id {
        let now = chrono::Local::now().naive_local();
        let model = context_summary::ActiveModel {
            save_id: Set(save_id),
            summary: Set(summary),
            cutoff_count: Set(cut as i32),
            updated_at: Set(now),
        };
        // 主键冲突即更新（一个存档一行）
        if context_summary::Entity::find_by_id(save_id)
            .one(&state.db)
            .await?
            .is_some()
        {
            model.update(&state.db).await?;
        } else {
            model.insert(&state.db).await?;
        }
    }

    let used_after = current_usage_tokens(&*gs_arc.lock().await);
    Ok(CompactOutcome {
        compacted_lines: cut,
        kept_lines: lines.len() - cut,
        used_tokens_after: used_after,
        message: format!(
            "已压缩 {} 条旧对话，保留最近 {} 条原文",
            cut,
            lines.len() - cut
        ),
    })
}

/// 自动压缩后台任务防重入：同一时间最多一个在跑。
static AUTO_COMPACT_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 在后台触发自动压缩（不阻塞发送链路）。
///
/// 发送消息时调用本函数即可：检查与压缩都在后台任务里做，当前这轮仍用
/// 旧上下文生成，压缩结果从下一轮开始生效（与 kimi-cli 轮间压缩同语义）。
/// 已有关闭配置、或已有压缩在跑时直接返回。
pub fn spawn_auto_compact_if_needed(app: &AppHandle) {
    if !crate::config::app_config::AppConfig::load(app)
        .map(|c| c.auto_compact)
        .unwrap_or(true)
    {
        return;
    }
    if AUTO_COMPACT_RUNNING.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        auto_compact_if_needed(&app).await;
        AUTO_COMPACT_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    });
}

/// 自动压缩检查：用量 ≥ 85% 窗口时压缩。仅由后台任务调用，严禁
/// 在发送链路同步 await（压缩含 LLM 调用，会卡住用户消息）。
/// 失败只记日志不打断对话。
pub async fn auto_compact_if_needed(app: &AppHandle) {
    if !crate::config::app_config::AppConfig::load(app)
        .map(|c| c.auto_compact)
        .unwrap_or(true)
    {
        return;
    }
    let state = app.state::<AppState>();
    let svc = state.ai_service.lock().await;
    let gs_arc = svc.game_status.clone();
    drop(svc);
    // 剧本演出期间不自动压缩（剧本台词不入共享历史的会被截断，避免误压剧本线）
    {
        let gs = gs_arc.lock().await;
        if gs.script_status.is_some() {
            return;
        }
    }
    let window = current_context_window(app).await;
    let used = current_usage_tokens(&*gs_arc.lock().await);
    if (used as f64) < window as f64 * AUTO_COMPACT_RATIO {
        return;
    }
    tracing::info!(
        "[ContextCompaction] 用量 {}/{} 超阈值，触发自动压缩",
        used,
        window
    );
    match compact_now(app, true).await {
        Ok(outcome) => {
            if outcome.compacted_lines > 0 {
                tracing::info!("[ContextCompaction] 自动压缩完成: {}", outcome.message);
            }
        },
        Err(error) => tracing::warn!("[ContextCompaction] 自动压缩失败（下轮再试）: {error}"),
    }
}

/// 回滚后内存态清理（纯逻辑，可单测）：锚点一律重置；cutoff 越界时
/// 清除摘要与 cutoff。返回是否清除了摘要。
fn reset_compaction_state_after_rollback(gs: &mut GameStatus) -> bool {
    gs.last_prompt_tokens = None;
    gs.last_usage_line_count = 0;
    if gs.context_summary_cutoff > gs.line_list.len() {
        gs.context_summary = None;
        gs.context_summary_cutoff = 0;
        return true;
    }
    false
}

/// 回滚截断后清理压缩状态。
///
/// cutoff 越界时旧摘要已失效（摘要覆盖了被截掉的台词分支），清空内存态并
/// 删除 DB 行，避免对话重新增长后旧摘要"复活"、把已放弃分支的剧情注回上下文；
/// 实测锚点来自被截掉的那轮生成，一律重置回退全量估算。
/// 调用方需已持有 game_status 锁（传入可变引用）。
pub async fn invalidate_summary_after_rollback(db: &DatabaseConnection, gs: &mut GameStatus) {
    if !reset_compaction_state_after_rollback(gs) {
        return;
    }
    if let Some(save_id) = gs.active_save_id {
        if let Err(error) = context_summary::Entity::delete_by_id(save_id)
            .exec(db)
            .await
        {
            tracing::warn!("[ContextCompaction] 回滚后删除失效摘要失败: {error}");
        }
    }
}

/// 从存档载入摘要到 GameStatus（读档/启动时调用）。
///
/// 调用方已持有 `ai_service` 锁（load_save 全程持有），本函数只接收
/// 拆开的 db / game_status 句柄，绝不回锁 ai_service，避免死锁。
pub async fn load_summary_into_status(
    db: &DatabaseConnection,
    gs_arc: &std::sync::Arc<tokio::sync::Mutex<GameStatus>>,
) -> Result<()> {
    let save_id = gs_arc.lock().await.active_save_id;
    let Some(save_id) = save_id else {
        return Ok(());
    };

    let row = context_summary::Entity::find_by_id(save_id).one(db).await?;
    let mut gs = gs_arc.lock().await;
    match row {
        Some(row) if row.cutoff_count > 0 && (row.cutoff_count as usize) <= gs.line_list.len() => {
            gs.context_summary = Some(row.summary);
            gs.context_summary_cutoff = row.cutoff_count as usize;
        },
        _ => {
            gs.context_summary = None;
            gs.context_summary_cutoff = 0;
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::reset_compaction_state_after_rollback;
    use crate::ai_service::game_system::game_status::GameStatus;
    use crate::ai_service::game_system::persistent_memory_system::MemorySectionLimits;
    use crate::ai_service::game_system::role_manager::GameRoleManager;
    use crate::ai_service::types::GameLine;
    use crate::config::tts::TtsConfig;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn status_with_lines(n: usize) -> GameStatus {
        // 测试不触碰 DB，但新签名要求一个 DatabaseConnection；用内存 sqlite 占位
        let db = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(sea_orm::Database::connect("sqlite::memory:"))
            .unwrap();
        let manager = GameRoleManager::new(
            PathBuf::new(),
            db,
            Arc::new(RwLock::new(None)),
            TtsConfig::default(),
            None,
            true,
            250,
            30,
            MemorySectionLimits::default(),
        );
        let mut gs = GameStatus::new(manager);
        for _ in 0..n {
            gs.line_list.push(GameLine::default());
        }
        gs
    }

    #[test]
    fn rollback_past_cutoff_clears_summary_and_anchor() {
        let mut gs = status_with_lines(10);
        gs.context_summary = Some("旧摘要".to_string());
        gs.context_summary_cutoff = 30; // 回滚后 len=10 < cutoff=30，摘要失效
        gs.last_prompt_tokens = Some(12_345);
        gs.last_usage_line_count = 40;

        assert!(reset_compaction_state_after_rollback(&mut gs));
        assert_eq!(gs.context_summary, None);
        assert_eq!(gs.context_summary_cutoff, 0);
        assert_eq!(gs.last_prompt_tokens, None);
        assert_eq!(gs.last_usage_line_count, 0);
    }

    #[test]
    fn rollback_above_cutoff_keeps_summary_but_resets_anchor() {
        let mut gs = status_with_lines(50);
        gs.context_summary = Some("仍有效摘要".to_string());
        gs.context_summary_cutoff = 30; // len=50 >= cutoff=30，摘要覆盖的台词未被截断
        gs.last_prompt_tokens = Some(12_345);
        gs.last_usage_line_count = 45;

        assert!(!reset_compaction_state_after_rollback(&mut gs));
        assert_eq!(gs.context_summary.as_deref(), Some("仍有效摘要"));
        assert_eq!(gs.context_summary_cutoff, 30);
        // 锚点来自被截掉的那轮生成，一律重置
        assert_eq!(gs.last_prompt_tokens, None);
        assert_eq!(gs.last_usage_line_count, 0);
    }

    #[test]
    fn rollback_without_summary_is_noop_but_resets_anchor() {
        let mut gs = status_with_lines(5);
        gs.last_prompt_tokens = Some(999);

        assert!(!reset_compaction_state_after_rollback(&mut gs));
        assert_eq!(gs.context_summary, None);
        assert_eq!(gs.context_summary_cutoff, 0);
        assert_eq!(gs.last_prompt_tokens, None);
    }
}
