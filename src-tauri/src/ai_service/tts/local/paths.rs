// Filesystem layout for local TTS assets.
//
// - `<app_data>/models/tts-local/`         root
// - `<app_data>/models/tts-local/assets/`  DeBerta + tokenizer shared assets
// - `<app_data>/models/tts-local/voices/`  one subdir per voice
// - `<app_cache>/tts-local-cache/`         temp (decompression, downloads)
//
// All resolution goes through `AppHandle::path()`. Android sandbox mapping is
// handled by tauri-plugin-android-fs (see `crate::init::static_copy`).

use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

// Logical assets that must be present for the local engine to function.
// Order matters: DeBerta is the first gate, the rest depend on it.
pub const REQUIRED_ASSETS: &[&str] = &["deberta"];

#[derive(Debug, Clone)]
pub struct LocalTtsPaths {
    pub root: PathBuf,
    pub assets: PathBuf,
    pub voices: PathBuf,
    pub cache: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VoiceInstallInfo {
    pub voice_id: String,
    pub path: PathBuf,
    pub size_bytes: u64,
}

impl LocalTtsPaths {
    pub fn resolve(app: &AppHandle) -> std::result::Result<Self, String> {
        let root = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("app_data_dir: {e}"))?
            .join("models")
            .join("tts-local");
        let assets = root.join("assets");
        let voices = root.join("voices");
        let cache = app
            .path()
            .app_cache_dir()
            .map_err(|e| format!("app_cache_dir: {e}"))?
            .join("tts-local-cache");
        Ok(Self { root, assets, voices, cache })
    }

    pub fn ensure(&self) -> std::result::Result<(), String> {
        for dir in [&self.root, &self.assets, &self.voices, &self.cache] {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("create_dir_all {}: {e}", dir.display()))?;
        }
        Ok(())
    }

    pub fn deberta_dir(&self) -> PathBuf {
        self.assets.join("deberta")
    }

    pub fn voice_dir(&self, voice_id: &str) -> PathBuf {
        self.voices.join(voice_id)
    }

    pub fn installed_voices(&self) -> std::result::Result<Vec<VoiceInstallInfo>, String> {
        if !self.voices.exists() {
            return Ok(vec![]);
        }
        let mut out = vec![];
        for entry in std::fs::read_dir(&self.voices)
            .map_err(|e| format!("read_dir voices: {e}"))?
        {
            let entry = entry.map_err(|e| format!("entry: {e}"))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let voice_id = entry.file_name().to_string_lossy().into_owned();
            let sbv2 = path.join("model.sbv2");
            let onnx = path.join("model.onnx");
            let primary = if sbv2.exists() { sbv2 } else { onnx };
            if !primary.exists() {
                continue;
            }
            let size = file_size(&primary).unwrap_or(0);
            out.push(VoiceInstallInfo { voice_id, path, size_bytes: size });
        }
        Ok(out)
    }

    pub fn asset_present(&self, asset_id: &str) -> bool {
        match asset_id {
            "deberta" => {
                let d = self.deberta_dir();
                d.join("deberta.onnx").exists() && d.join("tokenizer.json").exists()
            }
            _ => false,
        }
    }
}

fn file_size(p: &Path) -> std::io::Result<u64> {
    Ok(std::fs::metadata(p)?.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_assets_includes_deberta() {
        assert!(REQUIRED_ASSETS.contains(&"deberta"));
    }

    #[test]
    fn voice_dir_nests_under_voices() {
        let p = LocalTtsPaths {
            root: PathBuf::from("/tmp/x"),
            assets: PathBuf::from("/tmp/x/assets"),
            voices: PathBuf::from("/tmp/x/voices"),
            cache: PathBuf::from("/tmp/y"),
        };
        assert_eq!(p.voice_dir("alice"), PathBuf::from("/tmp/x/voices/alice"));
    }
}
