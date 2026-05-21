mod achievements;
mod ai_service;
mod api;
mod config;
mod db;
mod init;
mod migration;

use std::sync::Arc;

use sea_orm::DatabaseConnection;
use tauri::Manager;

use ai_service::llm::LlmClient;
use ai_service::message_system::processor::MessageProcessor;
use ai_service::service::SharedAIService;
use ai_service::translator::Translator;

pub struct ChatComponents {
    pub llm: Option<Arc<LlmClient>>,
    pub processor: Arc<MessageProcessor>,
    pub translator: Arc<Translator>,
}

pub struct AppState {
    pub db: DatabaseConnection,
    pub ai_service: SharedAIService,
    pub chat: ChatComponents,
    pub script_channels: ai_service::game_system::script_engine::SharedScriptChannels,
    pub generation_lock: Arc<tokio::sync::Mutex<()>>,
    pub proactive_system:
        Option<Arc<tokio::sync::Mutex<ai_service::proactive_system::ProactiveSystem>>>,
    pub achievement_manager:
        Arc<tokio::sync::Mutex<achievements::manager::AchievementManager>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        .setup(|app| {
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

            // Start proactive system loop
            let proactive_clone = proactive.clone();
            rt.spawn(async move {
                ai_service::proactive_system::ProactiveSystem::start(proactive_clone).await;
            });

            let achievement_manager = std::sync::Arc::new(tokio::sync::Mutex::new(
                achievements::manager::AchievementManager::new(&api::data_dir()),
            ));

            app.manage(AppState {
                db,
                ai_service,
                chat,
                script_channels,
                generation_lock,
                proactive_system: Some(proactive),
                achievement_manager,
            });

            // Spawn Windows mouse polling click-through loop
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| tauri::Error::AssetNotFound("main window not found".to_string()))?;

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
            config::get_settings_tree,
            config::save_settings,
            config::get_setting_by_key,
            config::select_file,
            api::character::get_character_list,
            api::character::get_role_info,
            api::character::get_role_settings,
            api::character::get_character_file,
            api::character::get_avatar_file,
            api::character::select_clothes,
            api::background::get_background_list,
            api::background::get_background_file,
            api::background::upload_background_image,
            api::background::open_backgrounds_folder,
            api::music::get_music_list,
            api::music::get_music_file,
            api::music::upload_music,
            api::media::get_script_media_file,
            api::asset::get_asset_base64,
            api::asset::get_voice_audio,
            api::game::init_game,
            api::game::select_character,
            api::game::reactivate_tts,
            api::chat::send_chat_message,
            api::save::list_saves,
            api::save::create_save,
            api::save::load_save,
            api::save::update_save,
            api::save::delete_save,
            api::script::list_scripts,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
