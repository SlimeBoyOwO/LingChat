//! Script event trait, registry, and execution context.
//!
//! Replaces Python's `BaseEvent` abstract class + `EventHandlerLoader` auto-discovery.
//! Rust does not have `importlib`, so event handlers register themselves via
//! `register_event()` and are looked up by `create_event()`.

// Event handler submodules
pub mod ai_dialogue_event;
pub mod background_effect_event;
pub mod background_event;
pub mod chapter_end_event;
pub mod choice_event;
pub mod dialog_event;
pub mod free_dialogue_event;
pub mod input_event;
pub mod modify_character_event;
pub mod music_event;
pub mod narration_event;
pub mod player_event;
pub mod present_pic_event;
pub mod set_variable_event;
pub mod ambient_event;
pub mod sound_event;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::ai_service::config::AIServiceConfig;
use crate::ai_service::game_system::game_status::GameStatus;
use crate::ai_service::llm::LlmClient;

// ============================================================
// Shared script channels (for user input/choice during scripts)
// ============================================================

/// Channels for user input/choice during script execution.
/// Stored as `Arc<Mutex<>>` so both the background task and Tauri commands
/// can access without holding the `AIService` lock.
pub struct ScriptChannels {
    pub input_tx: Option<tokio::sync::oneshot::Sender<String>>,
    pub choice_tx: Option<tokio::sync::oneshot::Sender<String>>,
}

impl ScriptChannels {
    pub fn new() -> Self {
        Self {
            input_tx: None,
            choice_tx: None,
        }
    }
}

pub type SharedScriptChannels = Arc<Mutex<ScriptChannels>>;

// ============================================================
// ScriptContext — bundled dependencies for event handlers
// ============================================================

/// All dependencies an event handler needs during execution.
pub struct ScriptContext<'a> {
    pub db: &'a DatabaseConnection,
    pub data_dir: &'a Path,
    pub app: &'a AppHandle,
    /// Owned Arc — events lock as needed. Decoupled from AIService lock
    /// so events can safely call MessageGenerator without deadlock.
    pub game_status: Arc<Mutex<GameStatus>>,
    pub config: &'a AIServiceConfig,

    /// Optional LLM client for `ai_dialogue`, `free_dialogue`, `chapter_end` (ai_judged).
    pub llm: Option<&'a Arc<LlmClient>>,

    /// Shared channels for user input/choice events.
    /// Owned `Arc` clone — handlers lock/unlock as needed around await points.
    pub channels: SharedScriptChannels,
}

// ============================================================
// ScriptEvent trait
// ============================================================

/// Trait for all script event handlers.
///
/// Each handler matches a YAML `type:` string and implements `execute()`.
/// Return `Ok(Some(next_chapter))` for chapter_end events; `Ok(None)` otherwise.
///
/// # Python parity note
///
/// Python `SetVariableEvent` overrode `execute()` instead of `_execute()`,
/// making it silently non-functional (base `process()` calls `_execute()`).
/// Rust uses a single `execute()` method — no such bug.
#[async_trait]
pub trait ScriptEvent: Send {
    /// Execute this event. Return `Some(chapter_name)` for chapter_end events.
    async fn execute(&mut self, ctx: &mut ScriptContext<'_>) -> Result<Option<String>>;

    /// The YAML `type:` string this handler matches (e.g. `"dialogue"`, `"narration"`).
    fn event_type() -> &'static str
    where
        Self: Sized;
}

// ============================================================
// Event registry
// ============================================================

pub type EventFactory = fn(event_data: Value) -> Box<dyn ScriptEvent>;

static REGISTRY: std::sync::LazyLock<RwLock<HashMap<&'static str, EventFactory>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

/// Register an event handler factory under a YAML `type:` string.
/// Called at startup by each event module.
pub fn register_event(event_type: &'static str, factory: EventFactory) {
    let mut registry = REGISTRY.write().expect("event registry poisoned");
    registry.insert(event_type, factory);
}

/// Create an event handler instance for the given YAML `type:` string.
/// The `event_data` is the raw YAML dict for this event.
/// Returns `None` if no handler is registered for that type.
pub fn create_event(event_type: &str, event_data: Value) -> Option<Box<dyn ScriptEvent>> {
    let registry = REGISTRY.read().expect("event registry poisoned");
    registry.get(event_type).map(|f| f(event_data))
}

// ============================================================
// Shared helpers
// ============================================================

/// Evaluate a condition expression against script variables.
/// Uses JSON value comparison for simple expressions like `flag == true`.
///
/// This is a simplified safe evaluator (no `eval()`). It supports:
/// - `var_name` alone (truthy check on the variable)
/// - `var_name == value` (equality)
/// - `var_name != value` (inequality)
pub fn evaluate_condition(condition: &str, vars: &serde_json::Map<String, Value>) -> bool {
    let condition = condition.trim();
    if condition.is_empty() {
        return true;
    }

    // Try `!=` first (longer pattern)
    if let Some((var, val)) = condition.split_once("!=") {
        let var = var.trim();
        let val = val.trim().trim_matches('"').trim_matches('\'');
        if let Some(current) = vars.get(var) {
            let current_str = match current {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            return current_str != val;
        }
        return true; // var not found → condition passes? Default to false
    }

    // Try `==`
    if let Some((var, val)) = condition.split_once("==") {
        let var = var.trim();
        let val = val.trim().trim_matches('"').trim_matches('\'');
        if let Some(current) = vars.get(var) {
            let current_str = match current {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            return current_str == val;
        }
        return false;
    }

    // Default: treat as bool variable lookup
    if let Some(current) = vars.get(condition) {
        match current {
            Value::Bool(b) => *b,
            Value::Null => false,
            _ => true, // non-null, non-bool → truthy
        }
    } else {
        false
    }
}
