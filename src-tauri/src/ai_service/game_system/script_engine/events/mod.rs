//! Script event trait, registry, and execution context.
//!
//! Replaces Python's `BaseEvent` abstract class + `EventHandlerLoader` auto-discovery.
//! Rust does not have `importlib`, so event handlers register themselves via
//! `register_event()` and are looked up by `create_event()`.

// Event handler submodules
pub mod ai_dialogue_event;
pub mod ambient_event;
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
    /// Whether the currently pending `choices` event accepts free-typed text.
    ///
    /// Mirrors the `allow_free` field of the in-flight [`choice_event::ChoiceEvent`].
    /// `script_submit_input` consults this so that text typed into the dialogue box
    /// while a choice is pending can resolve `choice_tx` instead of being rejected —
    /// without it the choice never resolves and the script blocks forever.
    pub choice_allow_free: bool,
}

impl ScriptChannels {
    pub fn new() -> Self {
        Self {
            input_tx: None,
            choice_tx: None,
            choice_allow_free: false,
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

    /// Skip every LLM call and substitute a visible placeholder line instead.
    ///
    /// Set only by the script editor's playtest. The author is debugging flow —
    /// event order, backgrounds, sprites, branching — and each AI turn costs
    /// real tokens for output they are about to scroll past. `false` everywhere
    /// else, so normal play is untouched.
    ///
    /// This is the "Rust 干跑" half of the agreed preview design: real engine,
    /// real render layer, but the expensive leaf nodes stubbed.
    pub dry_run_ai: bool,
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
///
/// Comparison is done on the string form of the value, so `flag == true`
/// matches a `Value::Bool(true)` as well as the string `"true"`.
///
/// # Undefined variables
///
/// An undefined variable is treated as "holds no value", which makes
/// `x == v` false and `x != v` true for every `v`. The two are consistent
/// with each other — do not "fix" one without the other.
///
/// # Not supported
///
/// `>`, `<`, `>=`, `<=`, `&&`, `||`, `!`, parentheses and arithmetic are **not**
/// implemented. `hp >= 5` does not compare anything: it falls through to the
/// bare-variable branch and looks up a variable literally named `"hp >= 5"`,
/// which never exists, so the condition is always false.
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
        // Undefined variable holds no value, so it differs from anything.
        // Mirrors the `==` branch below, which returns false in the same case.
        return true;
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

#[cfg(test)]
mod tests {
    use super::evaluate_condition;
    use serde_json::{json, Map, Value};

    fn vars(pairs: &[(&str, Value)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    #[test]
    fn empty_condition_always_passes() {
        assert!(evaluate_condition("", &Map::new()));
        assert!(evaluate_condition("   ", &Map::new()));
    }

    #[test]
    fn equality_compares_string_form() {
        let v = vars(&[
            ("flag", json!(true)),
            ("name", json!("钦灵")),
            ("count", json!(2)),
        ]);
        assert!(evaluate_condition("flag == true", &v));
        assert!(evaluate_condition("name == 钦灵", &v));
        assert!(evaluate_condition("name == \"钦灵\"", &v));
        assert!(evaluate_condition("count == 2", &v));
        assert!(!evaluate_condition("count == 3", &v));
    }

    #[test]
    fn inequality_is_the_complement_of_equality() {
        let v = vars(&[("route", json!("shop"))]);
        assert!(evaluate_condition("route != home", &v));
        assert!(!evaluate_condition("route != shop", &v));
    }

    /// Undefined variables must behave consistently across `==` and `!=`.
    /// Regression guard for the contradictory comment removed in PR1.
    #[test]
    fn undefined_variable_is_unequal_to_everything() {
        let v = Map::new();
        assert!(!evaluate_condition("missing == 1", &v));
        assert!(evaluate_condition("missing != 1", &v));
        assert!(!evaluate_condition("missing", &v));
    }

    #[test]
    fn bare_variable_is_a_truthiness_check() {
        let v = vars(&[
            ("t", json!(true)),
            ("f", json!(false)),
            ("n", Value::Null),
            ("s", json!("x")),
            ("zero", json!(0)),
        ]);
        assert!(evaluate_condition("t", &v));
        assert!(!evaluate_condition("f", &v));
        assert!(!evaluate_condition("n", &v));
        assert!(evaluate_condition("s", &v));
        // Note: 0 is "non-null, non-bool" and therefore truthy. Intentional.
        assert!(evaluate_condition("zero", &v));
    }

    /// Documents an intentional limitation so nobody assumes `>` works.
    #[test]
    fn comparison_operators_are_not_supported() {
        let v = vars(&[("hp", json!(10))]);
        assert!(!evaluate_condition("hp >= 5", &v));
        assert!(!evaluate_condition("hp > 5", &v));
    }
}
