//! 全局状态容器。
//!
//! `AppState` 是 Tauri 中 `manage` 的状态句柄，内部由 `OnceLock` 包裹
//! [`InnerAppState`]；启动建图（`app::setup::build::build_service_graph`）完成后
//! 一次性填充。`ChatComponents`
//! 与 `ScreenshotCaptureState` 是其中的组成部分。
//!
//! 本模块原先位于 crate 根（`lib.rs`），拆分后由 `lib.rs` 以 `pub use`
//! 重导出，保持 `crate::AppState` 等路径不变。

use std::sync::Arc;

use sea_orm::DatabaseConnection;

use crate::ai_service::god_agent::GodAgentCore;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::message_system::processor::MessageProcessor;
use crate::ai_service::screen_analyzer::ScreenAnalyzer;
use crate::ai_service::service::SharedAIService;
use crate::ai_service::tools::registry::ToolRegistry;
use crate::ai_service::translator::Translator;
use crate::{achievements, ai_service, api, plugins};

/// 聊天组件集合。
///
/// 包含聊天主 LLM 槽位、消息处理器和翻译器。
pub struct ChatComponents {
    /// 聊天主 LLM 槽位（支持运行时热切换）。
    /// 槽位本身始终存在，内部值可能为 None（表示尚未配置模型）。
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
    /// 插件管理器（扫描/启停/配置）。
    pub plugin_manager: Arc<plugins::PluginManager>,
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
    ///
    /// `editor_stop_preview` 会先唤醒被剧本阻塞的通道、把 `is_running` 置 false，
    /// 再立即 abort 这个句柄并还原共享 `GameStatus`。试玩任务即使被中止，其游离
    /// 流式任务（publisher/consumer）的迟到写入也会被 `preview_generation` 守卫
    /// 丢弃，`ai:reply` 则带 `preview_gen` 代号由前端比对丢弃（issue #5）。
    pub preview_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 当前剧本引擎任务的句柄：读档/新起跑时据此中止旧引擎，防止旧任务污染恢复后的状态。
    pub script_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 试玩开始时拍下的会话快照，供收尾时一次性还原。`Option::take` 保证幂等：
    /// 任务自然结束先还原、`editor_stop_preview` 兜底再 take 一次为空即跳过。
    pub pending_preview_restore:
        Arc<tokio::sync::Mutex<Option<api::script_editor::PreviewSession>>>,
}

/// AppState 在 Tauri 中 manage 的状态句柄。
///
/// **Android 修复**: Tauri 在 setup 闭包执行前就已经创建了 webview 窗口（见
/// `tauri::app::setup()`），前端 JS 一旦加载就会立刻 invoke 命令。如果用户的 setup
/// 闭包还在执行启动引导时，前端命令 `init_game` 在 IPC runtime worker 上
/// 被 dispatch 后调用 `state::<AppState>()` 就会 panic with
/// "state() called before manage()"。
///
/// 解决方案：setup 闭包**最开始**就 manage 一个空壳 AppState，
/// 启动建图完成后用真实值填充。`OnceLock` 提供一次性写入。
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
    ///
    /// rust-analyzer 无法解析 `State<AppState>` → `AppState` → `InnerAppState`
    /// 的双重 Deref 链，字段补全会失效。此方法将第二步 Deref 替换为方法调用，
    /// 通过 `state.data().ai_service` 即可正常触发补全。
    pub fn data(&self) -> &InnerAppState {
        self.inner
            .get()
            .expect("AppState accessed before initialization")
    }
}

/// 桌面端：简单 Deref，rust-analyzer 可以正确解析。
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
/// Tauri 在 Android 上会在 setup 闭包执行前就创建 webview 窗口，
/// 前端 JS 一旦加载就会立刻 invoke 命令。如果此时 panic，IPC worker
/// 线程会把整个进程拖死，所以必须自旋等待而非直接 panic。
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
