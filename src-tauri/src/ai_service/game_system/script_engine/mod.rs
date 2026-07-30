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
pub mod events;
pub mod events_handler;
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

    tracing::info!("[ScriptEngine] 所有事件处理器已注册");
}
