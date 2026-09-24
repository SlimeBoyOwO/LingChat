//! 服务图构建——**唯一**一处构造领域服务的地方。
//!
//! 合并自两处原先各自为政的建图代码：
//! - 原 `init/mod.rs::initialize()` 的后半段：聊天主/翻译 LLM 槽位、`AIService`、
//!   默认角色与服装覆盖、情绪分类器、`MessageProcessor`、`Translator`
//! - 原 `app/setup/services.rs` 的全部：工具注册表、插件管理器、主动系统、
//!   成就管理器、屏幕分析器、截图状态、自动存档管理器、上帝 Agent、Skill Agent
//!
//! 这两半本来就是同一件事——构建服务图——只是历史上被劈在了两个模块里。
//! `data.rs` 负责产出 `db` 与 `AppConfig`，本函数把它们连同 `local_tts` 一起
//! 变成填充 `AppState` 所需的 [`InnerAppState`]。
//!
//! **顺序即语义**：`db` / `ai_service` / `chat` 先被 `clone()` 给各服务，最后才按值
//! 移入 `InnerAppState`；任何重排都会破坏借用检查。

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use tauri::App;
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

use crate::ai_service::emotion::EmotionClassifier;
use crate::ai_service::game_system::persistent_memory_system::MemorySectionLimits;
use crate::ai_service::god_agent::GodAgentCore;
use crate::ai_service::god_agent::config::resolve_god_agent_provider;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, resolve_chat_provider, resolve_translate_provider,
};
use crate::ai_service::message_system::processor::{MessageProcessor, ProcessorOptions};
use crate::ai_service::screen_analyzer::{ScreenAnalyzer, ScreenAnalyzerConfig};
use crate::ai_service::service::{AIService, SharedAIService};
use crate::ai_service::translator::Translator;
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::types::CharacterSettings;
use crate::app::state::{ChatComponents, InnerAppState, ScreenshotCaptureState};
use crate::config::{self, AppConfig};
use crate::db;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;
use crate::{achievements, ai_service, api, plugins};

/// 建图结果。
pub(super) struct Services {
    /// 待 fill 进 `AppState` 的全部内部状态。
    pub inner: InnerAppState,
    /// 关闭处理器与定期循环复用同一实例——与 `inner.auto_save_manager` 是同一个 Arc。
    pub auto_save_manager:
        Arc<tokio::sync::Mutex<ai_service::game_system::auto_save::AutoSaveManager>>,
}

/// 构建整个领域服务图，返回填充 `AppState` 所需的全部内容。
///
/// **必须是同步函数**：仅把异步调用点交给传入的 `rt`（`rt.block_on(...)`）驱动。
/// 函数体里的 `PluginManager` 等代码会调用 `tokio::sync::Mutex::blocking_lock()`
/// 来阻塞当前线程，而 `blocking_lock()` 在 tokio runtime **内部**会 panic
/// （"Cannot block the current thread from within a runtime"）。所以本函数不能
/// 整体跑在 `rt.block_on` 里——重构前 `services::build` 也是在主线程上同步执行的，
/// 这里保持同一执行模型。
pub(super) fn build_service_graph(
    app: &tauri::App<tauri::Wry>,
    rt: &tokio::runtime::Runtime,
    db: DatabaseConnection,
    app_config: AppConfig,
    local_tts: Option<LocalTtsRuntime>,
) -> Result<Services> {
    let data_dir = api::data_dir();

    // ═══ 原 initialize() 后半：构建 AIService 与聊天组件 ═══
    // 构建聊天主 LLM 槽位（支持运行时热切换）。
    // 槽位本身始终存在，未配置模型时内部值为 None。
    let llm: LlmSlot = std::sync::Arc::new(tokio::sync::RwLock::new(
        resolve_chat_provider(&app.handle())
            .and_then(|p| build_llm_client_from_provider(&app.handle(), &p))
            .map(Arc::new),
    ));

    // AIService 内部的 GameRoleManager 共享同一个聊天 LLM 槽位
    let mut ai_service = rt.block_on(AIService::new(
        db.clone(),
        data_dir.clone(),
        llm.clone(),
        app_config.tts.clone(),
        local_tts,
        app_config.use_persistent_memory,
        app_config.memory_update_interval,
        app_config.memory_recent_window,
        MemorySectionLimits {
            short_term: app_config.memory_short_term_max_chars as usize,
            long_term: app_config.memory_long_term_max_chars as usize,
            user_info: app_config.memory_user_info_max_chars as usize,
            promises: app_config.memory_promises_max_chars as usize,
        },
        app_config.memory_inject_continue_user,
    ));

    // 加载默认角色：上次游玩的角色 → DB 中第一个主角色 → 默认空设定
    let settings = rt.block_on(load_default_character(app, &db, &data_dir))?;
    let character_id = settings.character_id;
    let prompt_options = PromptOptions {
        output_sec_lang: app_config.llm_output_sec_lang,
        no_emotion_limit: app_config.no_emotion_limit_prompt,
    };

    // 从 session store 读取各角色的上次服装，注入 GameRoleManager
    {
        let mut overrides = HashMap::new();
        if let Ok(store) = app.store(config::STORE_FILE) {
            if let Some(cid) = character_id {
                let key = config::session::last_clothes_key(cid);
                if let Some(clothes) = store.get(&key).and_then(|v| v.as_str().map(String::from)) {
                    if !clothes.is_empty() {
                        overrides.insert(cid, clothes);
                    }
                }
            }
        }
        rt.block_on(ai_service.set_clothes_overrides(overrides));
    }

    rt.block_on(ai_service.init_game_status(character_id, prompt_options))?;

    tracing::info!("AIService 初始化完成");

    let ai_service: SharedAIService = Arc::new(Mutex::new(ai_service));

    // —— 构建聊天组件 ——
    // 翻译 LLM 槽位（支持运行时热切换）；槽位本身始终存在。
    let translate_llm: LlmSlot = std::sync::Arc::new(tokio::sync::RwLock::new(
        resolve_translate_provider(&app.handle())
            .and_then(|p| build_llm_client_from_provider(&app.handle(), &p))
            .map(Arc::new),
    ));

    let classifier = load_emotion_classifier(app_config.enable_emotion_classifier, &data_dir);
    let processor = Arc::new(MessageProcessor::new(
        ProcessorOptions {
            time_sense_enabled: app_config.enable_time_sense,
            enable_translate: app_config.enable_translate,
        },
        classifier,
    ));

    let translator = Arc::new(Translator::new(
        translate_llm,
        !app_config.llm_output_sec_lang,
    ));

    let chat = ChatComponents {
        llm,
        processor,
        translator,
    };

    // ═══ 原 services.rs：构建其余全部服务 ═══
    // 创建脚本引擎通道
    let script_channels = std::sync::Arc::new(tokio::sync::Mutex::new(
        ai_service::game_system::script_engine::ScriptChannels::new(),
    ));

    // 创建生成锁
    let generation_lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
    let role_names = rt.block_on(db::managers::role_repo::RoleRepo::get_all_tool_role_names(
        &db,
    ))?;
    let tool_settings = ai_service::tools::settings::SharedToolSettings::new(
        ai_service::tools::settings::ToolSettings::load_or_create(&api::data_dir())?,
    );
    let tool_registry = Arc::new(ai_service::tools::built_in_registry(
        role_names,
        tool_settings.clone(),
    )?);

    // 插件系统：确保 data/plugins 目录存在并扫描加载插件（工具注册进 registry）。
    let plugin_manager = {
        let data_dir = api::data_dir();
        let plugins_root = data_dir.join("plugins");
        if std::fs::create_dir_all(&plugins_root).is_err() {
            tracing::warn!("插件目录创建失败: {}", plugins_root.display());
        }
        let manager = Arc::new(plugins::PluginManager::new(
            data_dir.clone(),
            tool_registry.clone(),
        ));
        // 插件注册可能更新了 available_tools，落盘到权限配置
        if let Err(e) = tool_registry.save_permissions(&data_dir) {
            tracing::warn!("插件注册后保存权限配置失败: {e}");
        }
        manager
    };

    // 创建主动系统
    let proactive = std::sync::Arc::new(tokio::sync::Mutex::new(
        ai_service::proactive_system::ProactiveSystem::new(
            app.handle().clone(),
            db.clone(),
            ai_service.clone(),
            ChatComponents {
                llm: chat.llm.clone(),
                processor: chat.processor.clone(),
                translator: chat.translator.clone(),
            },
            tool_registry.clone(),
            generation_lock.clone(),
        ),
    ));

    // 在 Tauri 运行时上启动主动系统循环
    let proactive_clone = proactive.clone();
    tauri::async_runtime::spawn(async move {
        ai_service::proactive_system::ProactiveSystem::start(proactive_clone).await;
    });

    // 创建成就管理器
    let achievement_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
        achievements::manager::AchievementManager::new(&api::data_dir()),
    ));

    // 创建屏幕分析器
    let screen_analyzer = {
        let sa_config = ScreenAnalyzerConfig::resolve(&app.handle());
        std::sync::Arc::new(tokio::sync::Mutex::new(ScreenAnalyzer::new(sa_config)))
    };

    // 创建截图捕获状态
    let screenshot_capture =
        std::sync::Arc::new(tokio::sync::Mutex::new(ScreenshotCaptureState::default()));

    // 创建自动存档管理器
    let auto_save_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
        ai_service::game_system::auto_save::AutoSaveManager::new(
            app.handle().clone(),
            db.clone(),
            ai_service.clone(),
        ),
    ));

    // 构建上帝 Agent（多人对话编排器）—— 使用独立槽位以支持热切换
    let god_agent = resolve_god_agent_provider(&app.handle()).map(|llm| {
        let config = ai_service::god_agent::config::GodAgentConfig::load(&app.handle());
        let slot: LlmSlot = std::sync::Arc::new(tokio::sync::RwLock::new(Some(Arc::new(llm))));
        Arc::new(GodAgentCore::new(slot, config))
    });

    // Skill Agent：确保技能库目录存在（兜底，不阻断启动）
    if let Err(e) = ai_service::skill_agent::ensure_skills_dir(&api::data_dir()) {
        tracing::warn!("Skill Agent 技能库目录初始化失败: {}", e);
    }

    // 组装 InnerAppState（原先直接 fill 进 AppState，现返回给编排层）
    let inner = InnerAppState {
        db,
        ai_service,
        chat,
        script_channels,
        generation_lock,
        tool_registry,
        tool_settings,
        plugin_manager,
        proactive_system: Some(proactive),
        achievement_manager,
        screen_analyzer,
        screenshot_capture,
        auto_save_manager: auto_save_manager.clone(),
        asr_state: Arc::new(ai_service::asr::AsrState {
            session: Arc::new(tokio::sync::Mutex::new(None)),
        }),
        god_agent,
        skill_agent: Arc::new(ai_service::skill_agent::SkillAgentState::default()),
        chat_command_approvals: Default::default(),
        chat_file_change_approvals: Default::default(),
        chat_file_delete_approvals: Default::default(),
        background_commands: Arc::new(
            ai_service::tools::background_command::BackgroundCommandManager::default(),
        ),
        preview_task: Arc::new(tokio::sync::Mutex::new(None)),
        script_task: Arc::new(tokio::sync::Mutex::new(None)),
        pending_preview_restore: Arc::new(tokio::sync::Mutex::new(None)),
    };

    Ok(Services {
        inner,
        auto_save_manager,
    })
}

// ═══ 私有 helper（原 init/mod.rs）═══

fn load_emotion_classifier(
    enabled: bool,
    data_dir: &std::path::Path,
) -> Option<Arc<EmotionClassifier>> {
    if !enabled {
        tracing::info!("情绪分类器已在配置中禁用");
        return None;
    }

    let model_dir = resolve_emotion_model_dir(data_dir);
    match model_dir {
        Some(dir) if dir.join("model.onnx").exists() => match EmotionClassifier::load(&dir) {
            Ok(clf) => {
                tracing::info!("情绪分类器加载成功: {}", dir.display());
                return Some(Arc::new(clf));
            },
            Err(e) => {
                tracing::warn!(
                    "情绪分类器加载失败 ({}), 回退为禁用状态: {e}",
                    dir.display()
                );
            },
        },
        _ => {
            tracing::warn!("未找到情绪模型目录, 情绪分类器将禁用");
        },
    }

    None
}

fn resolve_emotion_model_dir(data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    // 发布模式：data/emotion_model_19emo/
    let data_path = data_dir.join("third_party").join("emotion_model_19emo");
    if data_path.exists() {
        return Some(data_path);
    }

    None
}

/// 加载默认角色设定：上次游玩的角色 → 第一个主角色 → 默认空设定
async fn load_default_character(
    app: &App,
    db: &DatabaseConnection,
    data_dir: &std::path::Path,
) -> Result<CharacterSettings> {
    // 1. 尝试从 settings store 读取上次游玩的角色 ID
    let store = app
        .store(config::STORE_FILE)
        .unwrap_or_else(|_| app.handle().store(config::STORE_FILE).unwrap());
    if let Some(last_id) = store
        .get(config::session::LAST_CHARACTER_ID)
        .and_then(|v| v.as_i64())
    {
        if let Ok(Some(settings)) =
            RoleRepo::get_role_settings_by_id(db, data_dir, last_id as i32).await
        {
            tracing::info!("加载上次游玩的角色: id={}", last_id);
            return Ok(settings);
        }
    }

    // 2. 回退：取第一个主角色
    if let Ok(main_roles) = RoleRepo::get_all_main_roles(db).await {
        if let Some(role) = main_roles.first() {
            let folder = role.resource_folder.clone().unwrap_or_default();
            if let Ok(Some(settings)) =
                RoleRepo::get_role_settings_by_id(db, data_dir, role.id).await
            {
                tracing::info!("加载默认主角色: id={}, folder={}", role.id, folder);
                return Ok(settings);
            }
        }
    }

    // 3. 无角色可用时返回默认空设定
    tracing::warn!("无可用角色，使用默认空设定");
    Ok(CharacterSettings::default())
}
