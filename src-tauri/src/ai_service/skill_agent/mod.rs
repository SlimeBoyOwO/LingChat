//! Skill Agent：剧本编辑器里的 AI 助手。
//!
//! 能让 LLM 通过 `skills/` 技能库（SKILL.md）自动编写剧本 —— 具备文件读写、
//! shell 命令执行 + 用户审批、技能发现/读取能力。LLM 接入复用 LingChat 现有
//! `LlmClient`/provider 系统（镜像 God Agent），核心循环在 [`core::run_chat`]。
//!
//! 与游戏角色的工具系统（`tools/`）完全独立，不与 `tool_permissions.toml` 纠缠。

pub mod command_executor;
pub mod config;
pub mod core;
pub mod db;
pub mod events;
pub mod file_tools;
pub mod skills;
pub mod tools;

pub use command_executor::ApprovalMap;
pub use core::CancelFlag;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

/// 确保技能库目录存在（兜底；技能内容随 `data/game_data` 初始化）。
pub fn ensure_skills_dir(data_dir: &Path) -> std::io::Result<()> {
    let skills_dir = data_dir.join("game_data").join("skills");
    std::fs::create_dir_all(skills_dir)
}

/// Skill Agent 的共享可变状态（审批请求、取消标志、运行任务句柄）。
pub struct SkillAgentState {
    /// 待审批的命令：request_id → oneshot 发送端。
    pub approvals: ApprovalMap,
    /// 全局取消标志（跨一次运行共享）。
    pub cancelled: CancelFlag,
    /// 当前运行的后台任务句柄（`editor_agent_stop_chat` abort 用）。
    pub task: Arc<tokio::sync::Mutex<Option<tauri::async_runtime::JoinHandle<()>>>>,
}

impl Default for SkillAgentState {
    fn default() -> Self {
        Self {
            approvals: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            cancelled: Arc::new(AtomicBool::new(false)),
            task: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }
}
