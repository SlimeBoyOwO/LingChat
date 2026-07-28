use serde::{Deserialize, Serialize};

/// Most recent pet state pushed from the WebView.
/// Held so the Kotlin side can re-read it after the service restarts.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CachedPetState {
    pub character_id: Option<String>,
    pub avatar_url: Option<String>,
    pub expression: Option<String>,
    pub dialogue_text: Option<String>,
    pub dialogue_typing: Option<bool>,
    pub audio_playing: Option<bool>,
    pub scale: Option<f64>,
    pub volume: Option<u32>,
    pub visible: Option<bool>,
}
