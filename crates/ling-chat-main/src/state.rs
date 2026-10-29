//! 全局状态容器。
//!
//! `AppState` 是 Tauri 中 `manage` 的状态句柄，内部由 `OnceLock` 包裹
//! [`InnerAppState`]；启动建图（`app::setup::build::build_service_graph`）完成后
//! 一次性填充。`ChatComponents`
//! 与 `ScreenshotCaptureState` 是其中的组成部分。

use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::ai_service::god_agent::GodAgentCore;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::message_system::processor::MessageProcessor;
use crate::ai_service::screen_analyzer::ScreenAnalyzer;
use crate::ai_service::service::SharedAIService;
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::translator::Translator;
use crate::plugin_contract::PluginResourceSource;
use crate::{achievements, ai_service, api};

/// 聊天组件集合。
///
/// 包含聊天主 LLM 槽位、消息处理器和翻译器。
pub struct ChatComponents {
    /// 聊天主 LLM 槽位（支持运行时热切换）。
    pub llm: LlmSlot,
    /// 消息处理器，负责处理聊天消息的流转。
    pub processor: Arc<MessageProcessor>,
    /// 翻译 LLM 槽位（支持运行时热切换）。
    pub translator: Arc<Translator>,
}

/// 截图流程中的临时状态（全屏捕获 + 覆盖窗口标签）。
#[derive(Default)]
pub struct ScreenshotCaptureState {
    /// 全屏截图的 Base64 编码数据。
    pub full_capture_base64: Option<String>,
    /// 覆盖窗口的标签文本。
    pub overlay_label: Option<String>,
}

/// AppState 内部数据，启动建图完成后所有字段填充。
pub struct InnerAppState {
    /// 数据库连接实例。
    pub db: DatabaseConnection,
    /// AI 服务共享实例。
    pub ai_service: SharedAIService,
    /// 聊天组件。
    pub chat: ChatComponents,
    /// 脚本引擎通道。
    pub script_channels: ai_service::game_system::script_engine::SharedScriptChannels,
    /// 生成锁，用于控制并发生成。
    pub generation_lock: Arc<tokio::sync::Mutex<()>>,
    /// 主动系统实例（可选）。
    pub tool_registry: Arc<ToolRegistry>,
    /// 聊天工具的用户配置（网页搜索 API Key、代理等），热更新共享句柄。
    pub tool_settings: ai_service::tools::settings::SharedToolSettings,
    /// 文本嵌入后端（本地 ONNX 与云端的统一入口，内部含选路与熔断）。
    pub embedding: Arc<ai_service::embedding_remote::EmbeddingSelector>,
    /// 向量化预热任务句柄，同一时刻只允许一个在跑。
    pub embedding_warming: Arc<ai_service::embedding_remote::WarmingTask>,
    /// Tool RAG：检索工具并维护按角色的激活缓存。
    pub tool_rag: Arc<ai_service::tools::tool_rag::ToolRag>,
    /// 插件资源只读来源（由插件侧的 `PluginManager` 实现，宿主经窄接口读取）。
    pub plugin_resources: Arc<dyn PluginResourceSource>,
    pub proactive_system:
        Option<Arc<tokio::sync::Mutex<ai_service::proactive_system::ProactiveSystem>>>,
    /// 成就管理器。
    pub achievement_manager: Arc<tokio::sync::Mutex<achievements::manager::AchievementManager>>,
    /// 屏幕分析器。
    pub screen_analyzer: Arc<tokio::sync::Mutex<ScreenAnalyzer>>,
    /// 截图捕获状态。
    pub screenshot_capture: Arc<tokio::sync::Mutex<ScreenshotCaptureState>>,
    /// 自动存档管理器。
    pub auto_save_manager:
        Arc<tokio::sync::Mutex<ai_service::game_system::auto_save::AutoSaveManager>>,
    /// ASR 服务状态（详见 [`crate::ai_service::asr`]）。
    pub asr_state: Arc<ai_service::asr::AsrState>,
    /// 上帝 Agent（多人对话编排器，可选）。
    pub god_agent: Option<Arc<GodAgentCore>>,
    /// Skill Agent（剧本编辑器 AI 助手）共享状态。
    pub skill_agent: Arc<ai_service::skill_agent::SkillAgentState>,
    /// 主聊天 `execute_command` 工具的待审批命令请求（request_id → oneshot）。
    pub chat_command_approvals: ai_service::skill_agent::ApprovalMap,
    /// 主聊天 `write_file` / `edit_file` 工具的待审批修改请求。
    pub chat_file_change_approvals: ai_service::skill_agent::ApprovalMap,
    /// 主聊天 `delete_file` 工具的待审批删除请求（request_id → oneshot）。
    pub chat_file_delete_approvals: ai_service::skill_agent::ApprovalMap,
    /// 主聊天后台命令的并发槽位与任务 ID 分配器。
    pub background_commands: Arc<ai_service::tools::background_command::BackgroundCommandManager>,
    /// 剧本编辑器「试玩」当前在跑的后台任务句柄。
    pub preview_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 当前剧本引擎任务的句柄：读档/新起跑时据此中止旧引擎，防止旧任务污染恢复后的状态。
    pub script_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 试玩开始时拍下的会话快照
    pub pending_preview_restore:
        Arc<tokio::sync::Mutex<Option<api::script_editor::PreviewSession>>>,
}

/// AppState 在 Tauri 中 manage 的状态句柄。
pub struct AppState {
    inner: std::sync::OnceLock<InnerAppState>,
}

impl AppState {
    /// 创建一个空的 AppState 实例。
    pub fn empty() -> Self {
        Self {
            inner: std::sync::OnceLock::new(),
        }
    }

    /// 填充 AppState。只能调用一次。
    pub fn fill(&self, inner: InnerAppState) {
        if self.inner.set(inner).is_err() {
            panic!("AppState already filled (fill() must be called exactly once)");
        }
    }

    /// 直接返回内部数据引用，用于 IDE 补全。
    pub fn data(&self) -> &InnerAppState {
        self.inner
            .get()
            .expect("AppState accessed before initialization")
    }
}

/// Android 上的竞态窗口极小（manage → fill 只隔几行代码），桌面端从不触发。
#[cfg(not(target_os = "android"))]
impl std::ops::Deref for AppState {
    type Target = InnerAppState;
    fn deref(&self) -> &Self::Target {
        self.inner
            .get()
            .expect("AppState accessed before initialization")
    }
}

/// Android：spin-loop 等待 fill 完成。
#[cfg(target_os = "android")]
impl std::ops::Deref for AppState {
    type Target = InnerAppState;
    fn deref(&self) -> &Self::Target {
        loop {
            if let Some(inner) = self.inner.get() {
                return inner;
            }
            std::hint::spin_loop();
        }
    }
}
