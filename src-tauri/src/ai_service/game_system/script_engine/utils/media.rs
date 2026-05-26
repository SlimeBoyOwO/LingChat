//! Media file resolution for script events.
//!
//! Searches the current script's `Assets/` subdirectories first,
//! then falls back to global `game_data/` directories.

use std::path::Path;

/// Media type determines which subdirectories and fallback to search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Background,
    Music,
    Sound,
    Pic,
}

impl MediaType {
    /// Candidate subdirectory names under `Assets/` in the script directory.
    pub fn subdir_candidates(self) -> &'static [&'static str] {
        match self {
            MediaType::Background => &["Backgrounds", "Pics", "Pictures", "Pic", "Picture"],
            MediaType::Music => &["Musics", "BGMs", "Music", "BGM"],
            MediaType::Sound => &["Sounds", "SoundEffects", "Sound", "SoundEffect"],
            MediaType::Pic => &["Pics", "Pictures", "Pic", "Picture"],
        }
    }

    /// Fallback directory name under `game_data/`.
    pub fn fallback_dir(self) -> &'static str {
        match self {
            MediaType::Background | MediaType::Pic => "backgrounds",
            MediaType::Music | MediaType::Sound => "musics",
        }
    }
}

/// Resolve a script media file path from YAML event data.
///
/// Resolution order:
/// 1. If `script_path` is `Some`, search `{script_path}/Assets/{candidate}/{file_path}`
/// 2. Fallback: search `{data_dir}/game_data/{fallback_dir}/{file_path}`
///
/// Returns the canonical absolute path if found, `None` otherwise.
pub fn resolve_script_media(
    data_dir: &Path,
    script_path: Option<&Path>,
    file_path: &str,
    media_type: MediaType,
) -> Option<String> {
    if file_path.is_empty() {
        return None;
    }

    // Step 1: search current script's Assets directory
    if let Some(sp) = script_path {
        for candidate in media_type.subdir_candidates() {
            let candidate_path = sp.join("Assets").join(candidate).join(file_path);
            if candidate_path.exists() {
                if let Ok(canon) = candidate_path.canonicalize() {
                    return Some(canon.to_string_lossy().into_owned());
                }
            }
        }
    }

    // Step 2: fallback to game_data/{fallback_dir}
    let fallback_path = data_dir
        .join("game_data")
        .join(media_type.fallback_dir())
        .join(file_path);
    if fallback_path.exists() {
        if let Ok(canon) = fallback_path.canonicalize() {
            return Some(canon.to_string_lossy().into_owned());
        }
    }

    None
}
