//! Serializable payload types for Tauri script events.
//!
//! Each struct matches the payload shape expected by the frontend event processors
//! in `src/core/events/processors/`. The frontend's `asEvent()` helper merges in
//! `type` and `duration` fields, so those are omitted here.

use serde::Serialize;

// ============================================================
// Tauri event name constants
// ============================================================

pub mod event_names {
    pub const SCRIPT_NARRATION: &str = "script:narration";
    pub const SCRIPT_PLAYER: &str = "script:player";
    pub const SCRIPT_CHAPTER_CHANGE: &str = "script:chapter-change";
    pub const SCRIPT_BACKGROUND: &str = "script:background";
    pub const SCRIPT_BACKGROUND_EFFECT: &str = "script:background-effect";
    pub const SCRIPT_MUSIC: &str = "script:music";
    pub const SCRIPT_SOUND: &str = "script:sound";
    pub const SCRIPT_PRESENT_PIC: &str = "script:present-pic";
    pub const SCRIPT_MODIFY_CHARACTER: &str = "script:modify-character";
    pub const SCRIPT_INPUT: &str = "script:input";
    pub const SCRIPT_CHOICE: &str = "script:choice";
    pub const SCRIPT_END: &str = "script:end";
    pub const SCRIPT_FREE_DIALOGUE: &str = "script:free-dialogue";
}

// ============================================================
// Payload types (fields match frontend `src/types/script.ts`)
// ============================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NarrationPayload {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerPayload {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterChangePayload {
    pub chapter_name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundPayload {
    pub image_path: String,
    #[serde(default)]
    pub transition: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundEffectPayload {
    pub effect: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicPayload {
    pub music_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoundPayload {
    pub sound_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentPicPayload {
    pub image_path: String,
    #[serde(default = "default_scale")]
    pub scale: f64,
}

#[allow(dead_code)]
fn default_scale() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModifyCharacterPayload {
    pub character_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emotion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clothes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputPayload {
    pub hint: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChoicePayload {
    pub choices: Vec<String>,
    #[serde(default)]
    #[serde(rename = "allowFree")]
    pub allow_free: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeDialoguePayload {
    #[serde(rename = "switch")]
    pub switch: bool,
    pub max_rounds: i32,
    pub end_line: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptEndPayload {}
