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
            let rt = tokio::runtime::Runtime::new()?;
            let (db, ai_service, chat) = rt.block_on(init::initialize(app))?;
            app.manage(AppState { db, ai_service, chat });
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
            api::background::get_background_list,
            api::background::get_background_file,
            api::background::get_script_background_file,
            api::media::get_script_media_file,
            api::asset::get_asset_base64,
            api::asset::get_voice_audio,
            api::game::init_game,
            api::game::select_character,
            api::game::reactivate_tts,
            api::chat::send_chat_message,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
