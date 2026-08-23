//! Script engine — story/script mode execution.
//!
//! Replaces Python's `ling_chat/core/ai_service/script_engine/` package.
//!
//! Architecture:
//! - `ScriptManager` — script discovery, lifecycle, chapter orchestration
//! - `Chapter` — wraps a chapter YAML and runs its events
//! - `EventsHandler` — sequential event processor within a chapter
//! - `events` — event trait, registry, and all concrete event handlers
//! - `utils` — static helper functions for role lookup, variables, etc.
//! - `responses` — Tauri event payload types

pub mod chapter;
pub(crate) mod dlc_transaction;
pub mod events;
pub mod events_handler;
pub mod persistent_state;
pub(crate) mod reset_transaction;
pub mod responses;
pub mod script_manager;
pub mod utils;

// Re-export key types
pub use events::{ScriptChannels, SharedScriptChannels};
pub use script_manager::ScriptManager;

/// Initialize the script event registry by calling all event modules' `register()`.
/// Must be called once at startup before any scripts are run.
pub fn init_event_registry() {
    events::dialog_event::register();
    events::narration_event::register();
    events::player_event::register();
    events::poem_game_event::register();
    events::input_event::register();
    events::choice_event::register();
    events::ai_dialogue_event::register();
    events::free_dialogue_event::register();
    events::chapter_end_event::register();
    events::background_event::register();
    events::background_effect_event::register();
    events::music_event::register();
    events::sound_event::register();
    events::present_pic_event::register();
    events::modify_character_event::register();
    events::set_variable_event::register();
    // 注册环境音事件处理器
    events::ambient_event::register();
    // 注册成就解锁事件处理器
    events::achievement_event::register();
    // 注册恐怖演出事件处理器（突脸 / 强制选择）
    events::jumpscare_event::register();
    events::force_choice_event::register();
    // 注册定拍等待事件处理器（DDLC 式时间轴演出）
    events::wait_event::register();
    events::voice_shift_event::register();
    events::horror_log_event::register();
    events::random_var_event::register();
    // 剧本边界特效：持久菜单主题、外部角色标记、安全辅助窗口
    events::menu_effect_event::register();
    events::character_file_event::register();
    events::glitch_window_event::register();

    tracing::info!("[ScriptEngine] 所有事件处理器已注册");
}
