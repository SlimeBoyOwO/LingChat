// Filesystem layout for local TTS assets.
//
// - `<data_root>/models/tts-local/`         root
// - `<data_root>/models/tts-local/assets/`  DeBerta + tokenizer shared assets
// - `<data_root>/models/tts-local/voices/`  one subdir per voice
// - `<app_cache>/tts-local-cache/`          temp (decompression, downloads)

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[allow(dead_code)] // reserved for callers that validate all required local assets
pub const REQUIRED_ASSETS: &[&str] = &["deberta"];

#[derive(Debug, Clone)]
pub struct LocalTtsPaths {
    pub root: PathBuf,
    pub assets: PathBuf,
    pub voices: PathBuf,
    pub cache: PathBuf,
}

impl LocalTtsPaths {
    pub fn resolve(
        app: &AppHandle,
        desktop_data_root: PathBuf,
    ) -> std::result::Result<Self, String> {
        let data_root = resolve_models_root(app, desktop_data_root)?;
        let root = data_root.join("models").join("tts-local");
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
        crate::utils::path::ensure_dirs(&[&self.root, &self.assets, &self.voices, &self.cache])
    }

    pub fn deberta_dir(&self) -> PathBuf {
        self.assets.join("deberta")
    }

    pub fn voice_dir(&self, voice_id: &str) -> PathBuf {
        self.voices.join(voice_id)
    }

    pub fn style_vectors_path(&self, voice_id: &str) -> PathBuf {
        self.voices.join(voice_id).join("style_vectors.json")
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

fn resolve_models_root(
    _app: &AppHandle,
    _desktop_data_root: PathBuf,
) -> std::result::Result<PathBuf, String> {
    #[cfg(target_os = "android")]
    {
        use tauri_plugin_android_fs::{AndroidFsExt, AppDir};
        return _app
            .android_fs()
            .app_storage()
            .resolve_path(None, AppDir::Data)
            .map_err(|e| format!("android external files dir: {e}"));
    }

    #[cfg(not(target_os = "android"))]
    {
        Ok(_desktop_data_root)
    }
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

    #[test]
    fn deberta_presence_requires_model_and_tokenizer() {
        let temp = tempfile::tempdir().unwrap();
        let p = LocalTtsPaths {
            root: temp.path().join("tts-local"),
            assets: temp.path().join("tts-local/assets"),
            voices: temp.path().join("tts-local/voices"),
            cache: temp.path().join("cache"),
        };
        std::fs::create_dir_all(p.deberta_dir()).unwrap();
        std::fs::write(p.deberta_dir().join("deberta.onnx"), b"model").unwrap();
        assert!(!p.asset_present("deberta"));
        std::fs::write(p.deberta_dir().join("tokenizer.json"), b"{}").unwrap();
        assert!(p.asset_present("deberta"));
    }
}
