//! Skill Agent：剧本编辑器里的 AI 助手。

pub mod command_executor;
pub mod config;
pub mod core;
pub mod db;
pub mod events;
pub mod file_tools;
pub mod history;
pub mod role;
pub mod router;
pub mod skills;
pub mod stage;
pub mod title;
pub mod tools;

/// 对拍台：把喂给模型的文本逐字节钉住，等价重构靠它自证没改行为（只在测试里编译）。
pub use command_executor::ApprovalMap;
pub use core::CancelFlag;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

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
