mod achievements;
mod adventures;
mod ai_service;
mod api;
mod config;
mod db;
mod init;
mod migration;
mod utils;

use std::sync::Arc;

use chrono::Local;
use sea_orm::DatabaseConnection;
use tauri::Manager;
use tracing_subscriber::fmt::time::FormatTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

use ai_service::llm::LlmClient;
use ai_service::message_system::processor::MessageProcessor;
use ai_service::screen_analyzer::{ScreenAnalyzer, ScreenAnalyzerConfig};
use ai_service::service::SharedAIService;
use ai_service::translator::Translator;

struct LocalTimer;

impl FormatTime for LocalTimer {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", Local::now().format("%H:%M:%S"))
    }
}

pub struct ChatComponents {
    pub llm: Option<Arc<LlmClient>>,
    pub processor: Arc<MessageProcessor>,
    pub translator: Arc<Translator>,
}

/// 截图流程中的临时状态（全屏捕获 + 覆盖窗口标签）。
#[derive(Default)]
pub struct ScreenshotCaptureState {
    pub full_capture_base64: Option<String>,
    pub overlay_label: Option<String>,
}

pub struct AppState {
    pub db: DatabaseConnection,
    pub ai_service: SharedAIService,
    pub chat: ChatComponents,
    pub script_channels: ai_service::game_system::script_engine::SharedScriptChannels,
    pub generation_lock: Arc<tokio::sync::Mutex<()>>,
    pub proactive_system:
        Option<Arc<tokio::sync::Mutex<ai_service::proactive_system::ProactiveSystem>>>,
    pub achievement_manager: Arc<tokio::sync::Mutex<achievements::manager::AchievementManager>>,
    pub screen_analyzer: Arc<tokio::sync::Mutex<ScreenAnalyzer>>,
    pub screenshot_capture: Arc<tokio::sync::Mutex<ScreenshotCaptureState>>,
    pub auto_save_manager:
        Arc<tokio::sync::Mutex<ai_service::game_system::auto_save::AutoSaveManager>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn,ling_chat_lib=info"))
        .add_directive("sqlx=warn".parse().unwrap());

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_timer(LocalTimer)
                .with_filter(filter.clone()),
        )
        .with(
            utils::log_bridge::LogBridgeLayer
                .with_filter(filter),
        )
        .init();

    #[allow(deprecated)]
    unsafe {
        std::env::set_var(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS",
            "--force-color-profile=scrgb-linear",
        );
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_screenshots::init())
        .setup(|app| {
            utils::log_bridge::set_app_handle(app.handle().clone());

            app.manage(api::pet::HitTestState::default());

            let rt = tokio::runtime::Runtime::new()?;
            let (db, ai_service, chat) = rt.block_on(init::initialize(app))?;
            let script_channels = std::sync::Arc::new(tokio::sync::Mutex::new(
                ai_service::game_system::script_engine::ScriptChannels::new(),
            ));

            let generation_lock = std::sync::Arc::new(tokio::sync::Mutex::new(()));

            // Create proactive system
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
                    generation_lock.clone(),
                ),
            ));

            // Start proactive system loop on Tauri's runtime (NOT rt — rt is dropped when setup returns)
            let proactive_clone = proactive.clone();
            tauri::async_runtime::spawn(async move {
                ai_service::proactive_system::ProactiveSystem::start(proactive_clone).await;
            });

            let achievement_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
                achievements::manager::AchievementManager::new(&api::data_dir()),
            ));

            let screen_analyzer = {
                let pconfig = config::proactive::ProactiveConfig::load(&app.handle());
                let sa_config = ScreenAnalyzerConfig {
                    vd_api_key: pconfig.vd_api_key,
                    vd_base_url: pconfig.vd_base_url,
                    vd_model: pconfig.vd_model,
                };
                std::sync::Arc::new(tokio::sync::Mutex::new(ScreenAnalyzer::new(sa_config)))
            };

            let screenshot_capture = std::sync::Arc::new(tokio::sync::Mutex::new(
                ScreenshotCaptureState::default(),
            ));

            let auto_save_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
                ai_service::game_system::auto_save::AutoSaveManager::new(
                    app.handle().clone(),
                    db.clone(),
                    ai_service.clone(),
                ),
            ));

            app.manage(AppState {
                db,
                ai_service,
                chat,
                script_channels,
                generation_lock,
                proactive_system: Some(proactive),
                achievement_manager,
                screen_analyzer,
                screenshot_capture,
                auto_save_manager: auto_save_manager.clone(),
            });

            // Spawn Windows mouse polling click-through loop
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| tauri::Error::AssetNotFound("main window not found".to_string()))?;

            // Set up close handler for exit auto-save
            ai_service::game_system::auto_save::AutoSaveManager::setup_close_handler(
                app.handle().clone(),
                window.clone(),
                auto_save_manager.clone(),
            );

            // Start periodic auto-save loop (every 5 minutes)
            tauri::async_runtime::spawn(async move {
                ai_service::game_system::auto_save::AutoSaveManager::run_periodic(
                    auto_save_manager,
                )
                .await;
            });

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
        .invoke_handler(tauri::generate_handler![
            utils::log_bridge::get_log_history,
            config::get_settings_tree,
            config::save_settings,
            config::get_setting_by_key,
            config::select_file,
            config::list_llm_providers,
            config::save_llm_provider,
            config::delete_llm_provider,
            config::set_llm_role,
            config::test_llm_provider,
            api::character::get_character_list,
            api::character::get_role_info,
            api::character::get_role_settings,
            api::character::get_character_file,
            api::character::get_avatar_file,
            api::character::select_clothes,
            api::character::update_role_settings,
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

            api::asset::get_asset_base64,
            api::asset::get_voice_audio,
            api::game::init_game,
            api::game::select_character,
            api::game::reactivate_tts,
            api::chat::send_chat_message,
            api::chat::rollback_conversation,
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
            api::pet::update_solid_regions,
            api::pet::set_pet_mode,
            api::schedule::get_schedules,
            api::schedule::save_schedules,
            api::schedule::reload_proactive_system,
            api::achievement::get_achievement_list,
            api::achievement::unlock_achievement,
            api::adventure::list_character_adventures,
            api::adventure::list_all_adventures,
            api::adventure::start_adventure,
            api::adventure::check_adventure_unlocks,
            api::adventure::reset_adventure,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
