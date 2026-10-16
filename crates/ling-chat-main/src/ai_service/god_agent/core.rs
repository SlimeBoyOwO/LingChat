//! 上帝 Agent 核心：配置持有、任务运行与两个共享判定。
//!
//! 这里是各能力共用的基础设施：LLM 槽位、配置、工具注册表，加上「该不该激活」
//! 与「NPC 轮数还有没有额度」。具体能力（选人、好感度）各自在自己的模块里，
//! 副作用的落地在 `ai_service::tools::god_agent` 的工具里。
//!
//! 任务一律在**锁外**运行——决策工具内部会自己取 `game_status`，tokio 的 Mutex
//! 不可重入，持锁调用会自死锁。调用方负责先快照、再调用本模块。

use std::sync::Arc;

use anyhow::{Result, anyhow};
use tauri::AppHandle;

use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::god_agent::config::GodAgentConfig;
use crate::ai_service::llm::{LlmSlot, slot_snapshot};
use crate::ai_service::tools::agent::{AgentOutput, AgentTask, run_tool_agent};
use crate::ai_service::tools::god_agent::god_agent_registry;
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::types::GameLine;
use crate::db::entities::line::LineAttribute;

pub struct GodAgentCore {
    /// LLM 槽位（支持运行时热切换）。
    pub llm: LlmSlot,
    /// 运行配置（支持运行时热更新：save_settings 保存 god_agent/affection
    /// 相关设置后立即生效，无需重启；读写走 RwLock 快照）。
    pub config: std::sync::RwLock<GodAgentConfig>,
    /// 决策工具的专用注册表，与聊天注册表隔离（不进工具权限页）。
    registry: Arc<ToolRegistry>,
}

impl GodAgentCore {
    pub fn new(llm: LlmSlot, config: GodAgentConfig) -> Self {
        Self {
            llm,
            config: std::sync::RwLock::new(config),
            registry: Arc::new(god_agent_registry()),
        }
    }

    /// 运行时热更新配置（save_settings 保存相关设置后调用）。
    pub fn update_config(&self, config: GodAgentConfig) {
        *self.config.write().expect("上帝 Agent 配置锁中毒") = config.clone();
        tracing::info!(
            "[GodAgent] 配置已热更新: affection_enabled={}, eval_interval={}, recent_window={}, max_consecutive_npc={}",
            config.affection_enabled,
            config.affection_eval_interval,
            config.recent_window,
            config.max_consecutive_npc,
        );
    }

    /// 配置快照（读锁拷贝；GodAgentConfig 是小值类型，拷贝开销可忽略）。
    pub fn config_snapshot(&self) -> GodAgentConfig {
        self.config.read().expect("上帝 Agent 配置锁中毒").clone()
    }

    /// 取 LLM 槽位快照后无头运行一个任务。
    pub async fn run_task(&self, task: &AgentTask, app: &AppHandle) -> Result<AgentOutput> {
        let llm = slot_snapshot(&self.llm)
            .await
            .ok_or_else(|| anyhow!("上帝Agent LLM 未配置"))?;
        run_tool_agent(&llm, &self.registry, task, Some(app.clone())).await
    }

    // ============================================================
    // 共享判定
    // ============================================================

    /// 判断上帝 Agent 是否应在当前场景下激活。
    ///
    /// 条件：
    /// - 自由对话模式（`script_status.is_none()`）
    /// - 在场角色数 > 1（含玩家，即玩家 + 至少 1 个 NPC）
    pub fn should_activate(&self, gs: &GameStatus) -> bool {
        gs.script_status.is_none() && gs.present_role_ids.len() > 1
    }

    /// 连续 NPC 发言轮数是否仍在配置上限内。到顶后应由调用方交还玩家。
    pub fn within_npc_budget(&self, consecutive_npc_rounds: usize) -> bool {
        consecutive_npc_rounds < self.config_snapshot().max_consecutive_npc
    }
}

/// 取台词列表末尾 `window` 条，按由旧到新返回。各能力的视图快照共用。
///
/// 跳过 System 行：多人场景下每个角色的人设 prompt 都存在 line_list 的 System 行
/// 里，几千字符塞进决策上下文既浪费 token 又淹没对话信号；角色摘要已由视图的
/// subtitle/info 提供。旁白行（User 属性）保留。
pub(super) fn tail_lines(gs: &GameStatus, window: usize) -> Vec<GameLine> {
    gs.line_list
        .iter()
        .filter(|l| !matches!(l.attribute(), LineAttribute::System))
        .rev()
        .take(window)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}
