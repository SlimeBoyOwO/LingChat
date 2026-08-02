mod achievements;
mod adventures;
mod ai_service;
mod api;
mod config;
mod db;
mod init;
mod lan_sync;
mod manifest;
mod migration;
mod resource_sync;
pub mod utils;

use std::sync::Arc;

use chrono::Local;
use sea_orm::DatabaseConnection;
use tauri::Manager;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use ai_service::god_agent::config::resolve_god_agent_provider;
use ai_service::god_agent::GodAgentCore;
use ai_service::llm::LlmSlot;
use ai_service::message_system::processor::MessageProcessor;
use ai_service::screen_analyzer::{ScreenAnalyzer, ScreenAnalyzerConfig};
use ai_service::service::SharedAIService;
use ai_service::tools::registry::ToolRegistry;
use ai_service::translator::Translator;

/// 本地时间格式化器，用于日志输出的时间戳。
struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%H:%M:%S"))
    }
}

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

/// AppState 内部数据，init::initialize 完成后所有字段填充。
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
    /// 上帝 Agent（多人对话编排器，可选）。
    pub god_agent: Option<Arc<GodAgentCore>>,
    /// Skill Agent（剧本编辑器 AI 助手）共享状态。
    pub skill_agent: Arc<ai_service::skill_agent::SkillAgentState>,
    /// 剧本编辑器「试玩」当前在跑的后台任务句柄。
    ///
    /// `editor_stop_preview` 会先唤醒被剧本阻塞的通道、把 `is_running` 置 false，
    /// 再立即 abort 这个句柄并还原共享 `GameStatus`。试玩任务即使被中止，其游离
    /// 流式任务（publisher/consumer）的迟到写入也会被 `preview_generation` 守卫
    /// 丢弃，`ai:reply` 则带 `preview_gen` 代号由前端比对丢弃（issue #5）。
    pub preview_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 试玩开始时拍下的会话快照，供收尾时一次性还原。`Option::take` 保证幂等：
    /// 任务自然结束先还原、`editor_stop_preview` 兜底再 take 一次为空即跳过。
    pub pending_preview_restore: Arc<tokio::sync::Mutex<Option<api::script_editor::PreviewSession>>>,
}

/// AppState 在 Tauri 中 manage 的状态句柄。
///
/// **Android 修复**: Tauri 在 setup 闭包执行前就已经创建了 webview 窗口（见
/// `tauri::app::setup()`），前端 JS 一旦加载就会立刻 invoke 命令。如果用户的 setup
/// 闭包还在执行 init::initialize 时，前端命令 `init_game` 在 IPC runtime worker 上
/// 被 dispatch 后调用 `state::<AppState>()` 就会 panic with
/// "state() called before manage()"。
///
/// 解决方案：setup 闭包**最开始**就 manage 一个空壳 AppState，
/// init::initialize 完成后用真实值填充。`OnceLock` 提供一次性写入。
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 配置日志过滤器
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,ling_chat_lib=info"))
        .add_directive("sqlx=warn".parse().unwrap())
        .add_directive("genai=error".parse().unwrap());

    // 初始化日志系统
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_timer(LocalTimer)
                .with_filter(filter.clone()),
        )
        .with(utils::log_bridge::LogBridgeLayer.with_filter(filter.clone()))
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(utils::file_logger::LogFileWriter)
                .with_timer(LocalTimer)
                .with_ansi(false)
                .with_filter(filter),
        )
        .init();

    // 设置 WebView2 颜色配置文件（强制使用线性 sRGB）
    #[allow(deprecated)]
    unsafe {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--force-color-profile=scrgb-linear",
        );
    }

    // 构建 Tauri 应用
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_screenshots::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_android_fs::init());

    // 桌面端额外插件
    #[cfg(desktop)]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init());

    builder
        .setup(|app| {
            // 设置日志桥接的应用句柄
            utils::log_bridge::set_app_handle(app.handle().clone());

            // 提前初始化数据目录缓存，以便在 init::initialize 之前
            // 将其传递给独立的本地 TTS crate。
            init::static_copy::init_data_dir(&app.handle());

            // 管理各种状态
            app.manage(api::pet::HitTestState::default());
            app.manage(resource_sync::ResourceSyncState::default());
            app.manage(lan_sync::LanSyncState::default());
            app.manage(utils::cpu_perf::CpuDetectionCache::new());
            app.manage(api::role_archive::RoleArchiveState::default());

            // Android 修复：Tauri 在 setup 闭包执行前已创建 webview 窗口，前端 invoke
            // 命令会在 IPC runtime worker 上立即 dispatch；如果 AppState 还没 manage
            // 就会 panic "state() called before manage()"。所以 setup 一开始就 manage
            // 一个空壳 AppState，init::initialize 完成后用真实值 fill。
            app.manage(AppState::empty());
            let rt = tokio::runtime::Runtime::new()?;
            // 本地 TTS（SBV2 进程内实现）：解析路径、注册 State/开关并收敛运行时。
            let local_tts = ai_service::tts::local::setup::bootstrap(app)?;
            let (db, ai_service, chat) =
                rt.block_on(init::initialize(app, Some(local_tts.runtime.clone())))?;

            // 初始化文件日志（从设置读取开关和保留天数）
            {
                let store = config::settings_store(app.handle()).ok();
                let log_enable = store
                    .as_ref()
                    .and_then(|s| s.get(config::keys::LOG_ENABLE))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let retention_days = store
                    .as_ref()
                    .and_then(|s| s.get(config::keys::LOG_RETENTION_DAYS))
                    .and_then(|v| v.as_u64())
                    .map(|n| n as u32)
                    .unwrap_or(10);

                let data_dir = init::static_copy::get_data_dir();
                utils::file_logger::init_logging(data_dir, log_enable);
                utils::file_logger::cleanup_old_logs(retention_days);

                // 初始化 LLM 请求体日志（默认关闭）
                let llm_request_log_enable = store
                    .as_ref()
                    .and_then(|s| s.get(config::keys::LOG_LLM_REQUEST_BODY))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                utils::llm_request_logger::init(data_dir, llm_request_log_enable);
            }

            // 启动时自动清理未被引用的孤立语音文件
            match rt.block_on(init::voice_cleanup::cleanup_orphan_voice_files(
                &db,
                app.handle(),
            )) {
                Ok(stats) => {
                    tracing::info!("语音文件清理完成: 删除 {} 个文件", stats.deleted_count);
                }
                Err(e) => {
                    tracing::warn!("语音文件清理失败（非致命错误）: {e:#}");
                }
            }

            // 创建脚本引擎通道
            let script_channels = std::sync::Arc::new(tokio::sync::Mutex::new(
                ai_service::game_system::script_engine::ScriptChannels::new(),
            ));

            // 创建生成锁
            let generation_lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));
            let role_names = rt
                .block_on(db::managers::role_repo::RoleRepo::get_all_tool_role_names(&db))?;
            let tool_registry = Arc::new(ai_service::tools::built_in_registry(role_names)?);

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
                let slot: LlmSlot =
                    std::sync::Arc::new(tokio::sync::RwLock::new(Some(Arc::new(llm))));
                Arc::new(GodAgentCore::new(slot, config))
            });

            // Skill Agent：确保技能库目录存在（兜底，不阻断启动）
            if let Err(e) = ai_service::skill_agent::ensure_skills_dir(&api::data_dir()) {
                tracing::warn!("Skill Agent 技能库目录初始化失败: {}", e);
            }

            // 填充 AppState
            {
                let state = app.state::<AppState>();
                state.fill(InnerAppState {
                    db,
                    ai_service,
                    chat,
                    script_channels,
                    generation_lock,
                    tool_registry,
                    proactive_system: Some(proactive),
                    achievement_manager,
                    screen_analyzer,
                    screenshot_capture,
                    auto_save_manager: auto_save_manager.clone(),
                    god_agent,
                    skill_agent: Arc::new(ai_service::skill_agent::SkillAgentState::default()),
                    preview_task: Arc::new(tokio::sync::Mutex::new(None)),
                    pending_preview_restore: Arc::new(tokio::sync::Mutex::new(None)),
                });
            }

            // 延迟加载 DeBerta 直到应用主体挂载完成；
            // 如果在加载完成前有聊天请求到达，LocalTtsAdapter 的惰性引导仍然会运行，
            // 因此首次消息延迟是启动时加载的代价。
            ai_service::tts::local::setup::spawn_preload(&app.handle(), &local_tts);

            // 启动 Windows 鼠标轮询点击穿透循环
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| tauri::Error::AssetNotFound("main window not found".to_string()))?;

            // 设置退出自动存档的关闭处理器
            ai_service::game_system::auto_save::AutoSaveManager::setup_close_handler(
                app.handle().clone(),
                window.clone(),
                auto_save_manager.clone(),
            );

            // 启动定期自动存档循环（每 5 分钟）
            tauri::async_runtime::spawn(async move {
                ai_service::game_system::auto_save::AutoSaveManager::run_periodic(
                    auto_save_manager,
                )
                .await;
            });

            // Windows 点击穿透逻辑
            let hit_test_state = app.state::<api::pet::HitTestState>();
            let rects_arc = hit_test_state.solid_rects.clone();
            let enabled_arc = hit_test_state.enabled.clone();

            #[cfg(target_os = "windows")]
            {
                tauri::async_runtime::spawn(async move {
                    let mut was_ignored = false;
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

                        let enabled = if let Ok(locked) = enabled_arc.lock() {
                            *locked
                        } else {
                            false
                        };

                        if !enabled {
                            if was_ignored {
                                let _ = window.set_ignore_cursor_events(false);
                                was_ignored = false;
                            }
                            continue;
                        }

                        use windows::Win32::Foundation::POINT;
                        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

                        let mut pt = POINT { x: 0, y: 0 };
                        unsafe {
                            let _ = GetCursorPos(&mut pt);
                        }

                        if let Ok(window_pos) = window.outer_position() {
                            if let Ok(scale_factor) = window.scale_factor() {
                                let mouse_x = f64::from(pt.x) - f64::from(window_pos.x);
                                let mouse_y = f64::from(pt.y) - f64::from(window_pos.y);

                                let logical_x = mouse_x / scale_factor;
                                let logical_y = mouse_y / scale_factor;

                                let mut is_over_solid = false;
                                if let Ok(rects) = rects_arc.lock() {
                                    for r in rects.iter() {
                                        if logical_x >= r.x
                                            && logical_y >= r.y
                                            && logical_x <= (r.x + r.width)
                                            && logical_y <= (r.y + r.height)
                                        {
                                            is_over_solid = true;
                                            break;
                                        }
                                    }
                                }

                                if is_over_solid {
                                    if was_ignored {
                                        let _ = window.set_ignore_cursor_events(false);
                                        was_ignored = false;
                                    }
                                } else {
                                    if !was_ignored {
                                        let _ = window.set_ignore_cursor_events(true);
                                        was_ignored = true;
                                    }
                                }
                            }
                        }
                    }
                });
            }

            Ok(())
        })
        // 注册所有 API 命令
        .invoke_handler(tauri::generate_handler![
            utils::log_bridge::get_log_history,
            utils::log_bridge::open_log_window,
            utils::log_bridge::is_log_window_open,
            api::settings::get_settings_tree,
            api::settings::save_settings,
            api::settings::get_setting_by_key,
            api::settings::select_file,
            api::settings::list_llm_providers,
            api::settings::save_llm_provider,
            api::settings::delete_llm_provider,
            api::settings::set_llm_role,
            api::settings::switch_llm,
            api::settings::test_llm_provider,
            api::settings::list_llm_models,
            api::font::list_system_fonts,
            api::font::import_font,
            api::font::list_imported_fonts,
            api::font::delete_imported_font,
            api::character::get_character_list,
            api::character::get_role_info,
            api::character::get_role_settings,
            api::character::get_character_file,
            api::character::get_avatar_file,
            api::character::select_clothes,
            api::character::update_role_settings,
            api::character::open_characters_folder,
            api::background::get_background_list,
            api::background::get_background_file,
            api::background::upload_background_image,
            api::background::open_backgrounds_folder,
            api::scene::list_scenes,
            api::scene::create_scene,
            api::scene::update_scene,
            api::scene::delete_scene,
            api::scene::select_scene,
            api::scene::set_scene_awareness,
            api::music::get_music_list,
            api::music::get_music_file,
            api::music::upload_music,
            api::music::delete_music,
            api::music::save_bgm_state,
            api::locale::get_locale_messages,
            api::ambient::get_ambient_list,
            api::ambient::upload_ambient,
            api::ambient::delete_ambient,
            api::ambient::save_ambient_state,
            api::asset::get_asset_base64,
            api::asset::get_voice_audio,
            api::game::init_game,
            api::game::select_character,
            api::game::clear_conversation,
            api::game::reactivate_tts,
            api::game::clear_tts_cache,
            api::game::update_voice_lang,
            api::game::get_tts_cache_info,
            api::game::add_role_to_scene,
            api::game::remove_role_from_scene,
            api::game::notify_player_entry,
            api::chat::send_chat_message,
            api::chat::rollback_conversation,
            api::chat::feed_image,
            api::chat::feed_text,
            api::screenshot::start_screenshot,
            api::screenshot::get_overlay_data,
            api::screenshot::confirm_screenshot,
            api::screenshot::cancel_screenshot,
            api::save::list_saves,
            api::save::create_save,
            api::save::load_save,
            api::save::update_save,
            api::save::delete_save,
            api::save::update_save_title,
            api::save::save_screenshot,
            api::save::capture_main_window_screenshot,
            api::script::list_scripts,
            api::script::list_standalone_scripts,
            api::script::start_script,
            api::script::script_submit_input,
            api::script::script_submit_choice,
            // ── 剧本编辑器 ──
            api::script_editor::editor_get_schema,
            api::script_editor::editor_list_scripts,
            api::script_editor::editor_read_script,
            api::script_editor::editor_read_chapter,
            api::script_editor::editor_validate_script,
            api::script_editor::editor_write_chapter,
            api::script_editor::editor_write_story_config,
            api::script_editor::editor_create_chapter,
            api::script_editor::editor_delete_chapter,
            api::script_editor::editor_delete_character,
            api::script_editor::editor_create_script,
            api::script_editor::editor_delete_script,
            api::script_editor::editor_upload_asset,
            api::script_editor::editor_create_character,
            api::script_editor::editor_list_global_assets,
            api::script_editor::editor_list_asset_files,
            api::script_editor::editor_delete_asset,
            api::script_editor::editor_rescan_scripts,
            api::script_editor::editor_start_preview,
            api::script_editor::editor_preview_readiness,
            api::script_editor::editor_list_global_characters,
            api::script_editor::editor_import_global_character,
            api::script_editor::editor_stop_preview,
            api::script_editor::editor_open_script_folder,
            // ── 剧本编辑器 · AI 助手（Skill Agent）──
            api::script_editor::agent::editor_agent_get_settings,
            api::script_editor::agent::editor_agent_save_settings,
            api::script_editor::agent::editor_agent_get_default_dirs,
            api::script_editor::agent::editor_agent_list_skills,
            api::script_editor::agent::editor_agent_read_skill,
            api::script_editor::agent::editor_agent_create_conversation,
            api::script_editor::agent::editor_agent_list_conversations,
            api::script_editor::agent::editor_agent_delete_conversation,
            api::script_editor::agent::editor_agent_get_messages,
            api::script_editor::agent::editor_agent_clear_conversation,
            api::script_editor::agent::editor_agent_start_chat,
            api::script_editor::agent::editor_agent_stop_chat,
            api::script_editor::agent::editor_agent_resolve_approval,
            api::pet::update_solid_regions,
            api::pet::set_pet_mode,
            api::schedule::get_schedules,
            api::schedule::save_schedules,
            api::schedule::reload_proactive_system,
            api::proactive_set_can_deliver,
            api::achievement::get_achievement_list,
            api::achievement::unlock_achievement,
            api::adventure::list_character_adventures,
            api::adventure::list_all_adventures,
            api::adventure::start_adventure,
            api::adventure::check_adventure_unlocks,
            api::adventure::reset_adventure,
            api::workshop::fetch_discussions,
            resource_sync::check_resource_sync,
            resource_sync::apply_resource_sync,
            resource_sync::get_data_version,
            lan_sync::lan_sync_start_server,
            lan_sync::lan_sync_stop_server,
            lan_sync::lan_sync_scan_peers,
            lan_sync::lan_sync_plan_push,
            lan_sync::lan_sync_execute_push,
            lan_sync::lan_sync_plan_pull,
            lan_sync::lan_sync_execute_pull,
            lan_sync::lan_sync_restart,
            utils::cpu_perf::get_cpu_info,
            utils::cpu_perf::redetect_cpu,
            api::role_archive::import_role,
            api::role_archive::import_role_from_path,
            api::role_archive::cancel_role_import,
            api::role_archive::rescan_roles,
            api::role_archive::export_role,
            api::role_archive::export_role_to_path,
            // 本地 TTS 相关命令
            ai_service::tts::local::tts_local_status,
            ai_service::tts::local::tts_local_list_catalog,
            ai_service::tts::local::tts_local_list_installed,
            ai_service::tts::local::tts_local_import_from_path,
            ai_service::tts::local::tts_local_download,
            ai_service::tts::local::tts_local_delete_voice,
            ai_service::tts::local::tts_local_import_style_vectors,
            ai_service::tts::local::tts_local_synthesize_preview,
            ai_service::tts::local::tts_local_get_enabled,
            ai_service::tts::local::tts_local_set_enabled,
            exit_app,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 前端确认关闭后调用，终止整个 Tauri 进程。
#[tauri::command]
fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

