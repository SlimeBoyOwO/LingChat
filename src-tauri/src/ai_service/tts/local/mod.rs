// Local TTS engine module (formerly the sbv2-local-tts crate, now embedded).

pub mod adapter;
pub mod engine;
pub mod model_manager;
pub mod package;
pub mod paths;
pub mod registry;

mod download;
mod saf_bridge;

use std::sync::Arc;

pub use engine::{LocalTtsEngine, SynthesizeRequest};
pub use paths::LocalTtsPaths;

// ---------------------------------------------------------------------------
// LocalTtsState -- Tauri managed state for the local TTS engine
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};
use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

pub struct LocalTtsState {
    pub paths: LocalTtsPaths,
    pub engine: Arc<LocalTtsEngine>,
    pub cancel: tokio::sync::Mutex<Option<Arc<CancellationToken>>>,
}

impl LocalTtsState {
    pub fn new(paths: LocalTtsPaths) -> Self {
        Self {
            paths,
            engine: Arc::new(LocalTtsEngine::new()),
            cancel: tokio::sync::Mutex::new(None),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct TtsLocalStatus {
    pub ready: bool,
    pub deberta_installed: bool,
    pub installed_voice_count: usize,
}

#[derive(Debug, Serialize)]
pub struct TtsLocalInstallSnapshot {
    pub assets: Vec<model_manager::AssetRecord>,
    pub voices: Vec<model_manager::VoiceRecord>,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub asset_id: String,
    pub voice_id: Option<String>,
    pub path: String,
    pub bytes: u64,
    pub message: String,
}

// ---------------------------------------------------------------------------
// LocalTtsSwitch -- runtime enable/disable gate
// ---------------------------------------------------------------------------

use std::sync::atomic::{AtomicBool, Ordering};

use crate::config;

#[derive(Clone, Debug)]
pub struct LocalTtsSwitch {
    enabled: Arc<AtomicBool>,
}

impl LocalTtsSwitch {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct LocalTtsSwitchStatus {
    pub configured_enabled: bool,
    pub effective_enabled: bool,
}

fn read_configured_enabled(app: &AppHandle) -> Result<bool, String> {
    let store = config::settings_store(app).map_err(|e| e.to_string())?;
    Ok(store
        .get(config::keys::ENABLE_LOCAL_TTS)
        .and_then(|value| value.as_bool())
        .unwrap_or(false))
}

pub fn load_configured_enabled(app: &AppHandle) -> bool {
    read_configured_enabled(app).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Tauri commands -- switch management
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn tts_local_get_enabled(
    app: AppHandle,
    switch: State<'_, LocalTtsSwitch>,
) -> Result<LocalTtsSwitchStatus, String> {
    Ok(LocalTtsSwitchStatus {
        configured_enabled: read_configured_enabled(&app)?,
        effective_enabled: switch.is_enabled(),
    })
}

#[tauri::command]
pub fn tts_local_set_enabled(
    app: AppHandle,
    switch: State<'_, LocalTtsSwitch>,
    local_state: State<'_, LocalTtsState>,
    enabled: bool,
) -> Result<LocalTtsSwitchStatus, String> {
    if enabled {
        local_state.paths.ensure()?;
    }

    let store = config::settings_store(&app).map_err(|e| e.to_string())?;
    let previous = store.get(config::keys::ENABLE_LOCAL_TTS);
    store.set(config::keys::ENABLE_LOCAL_TTS, enabled);
    if let Err(error) = store.save() {
        if let Some(value) = previous {
            store.set(config::keys::ENABLE_LOCAL_TTS, value);
        } else {
            store.delete(config::keys::ENABLE_LOCAL_TTS);
        }
        return Err(format!("save local TTS switch: {error}"));
    }

    switch.set_enabled(enabled);
    Ok(LocalTtsSwitchStatus {
        configured_enabled: enabled,
        effective_enabled: enabled,
    })
}

// ---------------------------------------------------------------------------
// Tauri commands -- engine operations (from the former crate)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn tts_local_status(
    state: State<'_, LocalTtsState>,
) -> Result<TtsLocalStatus, String> {
    let voices = model_manager::list_voices(&state.paths)?;
    let deberta_installed = state.paths.asset_present("deberta");
    Ok(TtsLocalStatus {
        ready: state.engine.is_ready().await,
        deberta_installed,
        installed_voice_count: voices.len(),
    })
}

#[tauri::command]
pub async fn tts_local_list_catalog() -> Result<Vec<registry::AssetEntry>, String> {
    let all = registry::all_assets();
    // 收集所有被其他条目捆绑的资产 ID，在前端列表中隐藏它们
    let bundled: std::collections::HashSet<String> = all
        .iter()
        .flat_map(|a| a.bundled_assets.iter().cloned())
        .collect();
    Ok(all
        .into_iter()
        .filter(|a| !bundled.contains(&a.id))
        .collect())
}

#[tauri::command]
pub async fn tts_local_list_installed(
    state: State<'_, LocalTtsState>,
) -> Result<TtsLocalInstallSnapshot, String> {
    Ok(TtsLocalInstallSnapshot {
        assets: model_manager::list_assets(&state.paths)?,
        voices: model_manager::list_voices(&state.paths)?,
    })
}

// -- helpers ----------------------------------------------------------------

fn install_shared_asset(
    paths: &LocalTtsPaths,
    src: &Path,
    asset_id: &str,
) -> Result<PathBuf, String> {
    let (target, _label) = match asset_id {
        "deberta" => (paths.deberta_dir().join("deberta.onnx"), "DeBERTa model"),
        "deberta-tokenizer" => (paths.deberta_dir().join("tokenizer.json"), "DeBERTa tokenizer"),
        other => return Err(format!("unknown shared asset: {other}")),
    };
    crate::utils::fs::copy_with_parent(src, &target)
}

fn install_style_vectors_for(
    paths: &LocalTtsPaths,
    src: &Path,
    voice_id: &str,
) -> Result<PathBuf, String> {
    crate::utils::fs::copy_with_parent(src, &paths.voice_dir(voice_id).join("style_vectors.json"))
}

fn shared_asset_file_name(asset_id: &str) -> Result<&'static str, String> {
    match asset_id {
        "deberta" => Ok("deberta.onnx"),
        "deberta-tokenizer" => Ok("tokenizer.json"),
        other => Err(format!("unknown BERT asset: {other}")),
    }
}

fn download_temp_path(entry: &registry::AssetEntry, cache: &Path) -> PathBuf {
    let ext = registry::expected_extension(entry);
    cache.join(format!("{}.download.{ext}", entry.id))
}

fn default_voice_id(
    _inspected: &package::InspectedPackage,
    src: &Path,
) -> String {
    let stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("voice");
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches('-').to_lowercase();
    if cleaned.is_empty() {
        "voice".into()
    } else {
        cleaned
    }
}

// -- import / download / delete ---------------------------------------------

#[tauri::command]
pub async fn tts_local_import_from_path(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    path: String,
    voice_id: Option<String>,
    asset_id: Option<String>,
) -> Result<ImportResult, String> {
    let (src, cleanup_after_import) =
        saf_bridge::prepare_file_import_source(&app, &path).await?;

    let result: std::result::Result<ImportResult, String> = async {
        if let Some(asset_id) = asset_id {
            let installed = install_shared_asset(&state.paths, &src, &asset_id)?;
            let bytes = std::fs::metadata(&installed)
                .map(|m| m.len())
                .unwrap_or(0);
            if state.paths.asset_present("deberta") {
                let _ = state.engine.init(&state.paths).await;
            }
            let _ = app.emit("tts://install-complete", &asset_id);
            return Ok(ImportResult {
                asset_id: asset_id.clone(),
                voice_id: None,
                path: installed.to_string_lossy().into_owned(),
                bytes,
                message: "shared asset imported".into(),
            });
        }

        if !src.exists() {
            return Err(format!("path not found: {}", src.display()));
        }
        let inspected = package::inspect_package(&src)?;
        let voice_id = match voice_id {
            Some(v) => v,
            None => default_voice_id(&inspected, &src),
        };
        let installed =
            package::install_inspected(&inspected, &src, &state.paths, &voice_id)?;
        let bytes = std::fs::metadata(&installed)
            .map(|m| m.len())
            .unwrap_or(0);
        let _ = app.emit("tts://install-complete", &voice_id);
        Ok(ImportResult {
            asset_id: voice_id.clone(),
            voice_id: Some(voice_id),
            path: installed.to_string_lossy().into_owned(),
            bytes,
            message: "imported".into(),
        })
    }
    .await;

    if cleanup_after_import {
        let _ = tokio::fs::remove_file(&src).await;
    }
    result
}

#[tauri::command]
pub async fn tts_local_download(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    asset_id: String,
) -> Result<Vec<ImportResult>, String> {
    let entry = registry::find(&asset_id)
        .ok_or_else(|| format!("asset {asset_id} not in catalog"))?;

    let cancel = Arc::new(CancellationToken::new());
    {
        let mut guard = state.cancel.lock().await;
        *guard = Some(cancel.clone());
    }

    // 收集所有需要下载的资产：主资产 + 捆绑资产
    let bundled_ids = entry.bundled_assets.clone();
    let mut to_download: Vec<registry::AssetEntry> = vec![entry];
    for bundled_id in &bundled_ids {
        if let Some(e) = registry::find(bundled_id) {
            to_download.push(e);
        }
    }

    let result = async {
        let mut results: Vec<ImportResult> = Vec::new();
        for entry in &to_download {
            let r = download_single_asset(&app, &state, entry, cancel.clone()).await?;
            results.push(r);
        }
        // DeBERTa 全套就位时初始化引擎
        if state.paths.asset_present("deberta") {
            let _ = state.engine.init(&state.paths).await;
        }
        Ok::<_, String>(results)
    }
    .await;

    {
        let mut guard = state.cancel.lock().await;
        *guard = None;
    }
    let _ = app.emit("tts://download-complete", &asset_id);
    result
}

/// 下载单个资产（Bert/Voice/StyleVectors），返回 ImportResult。
async fn download_single_asset(
    app: &AppHandle,
    state: &LocalTtsState,
    entry: &registry::AssetEntry,
    cancel: Arc<CancellationToken>,
) -> Result<ImportResult, String> {
    match entry.kind {
        registry::AssetKind::Bert => {
            let file_name = shared_asset_file_name(&entry.id)?;
            let dst = state.paths.deberta_dir().join(file_name);
            std::fs::create_dir_all(state.paths.deberta_dir())
                .map_err(|e| format!("mkdir deberta: {e}"))?;
            let bytes = download::download_asset(app, entry, &dst, cancel).await?;
            Ok(ImportResult {
                asset_id: entry.id.clone(),
                voice_id: None,
                path: dst.to_string_lossy().into_owned(),
                bytes,
                message: format!("{} downloaded", entry.id),
            })
        }
        registry::AssetKind::Voice => {
            let raw_dst = download_temp_path(entry, &state.paths.cache);
            let bytes = download::download_asset(app, entry, &raw_dst, cancel).await?;
            let inspected = package::inspect_package(&raw_dst)?;
            let installed = package::install_inspected(
                &inspected,
                &raw_dst,
                &state.paths,
                &entry.id,
            )?;
            let _ = tokio::fs::remove_file(&raw_dst).await;
            Ok(ImportResult {
                asset_id: entry.id.clone(),
                voice_id: Some(entry.id.clone()),
                path: installed.to_string_lossy().into_owned(),
                bytes,
                message: "voice downloaded".into(),
            })
        }
        registry::AssetKind::StyleVectors => {
            let voice_id = entry.voice_id.clone().ok_or_else(|| {
                format!("style_vectors asset {} missing voice_id", entry.id)
            })?;
            let raw_dst = download_temp_path(entry, &state.paths.cache);
            let bytes = download::download_asset(app, entry, &raw_dst, cancel).await?;
            let installed =
                install_style_vectors_for(&state.paths, &raw_dst, &voice_id)?;
            let _ = tokio::fs::remove_file(&raw_dst).await;
            Ok(ImportResult {
                asset_id: entry.id.clone(),
                voice_id: Some(voice_id.clone()),
                path: installed.to_string_lossy().into_owned(),
                bytes,
                message: "style vectors downloaded".into(),
            })
        }
    }
}

#[tauri::command]
pub async fn tts_local_delete_voice(
    state: State<'_, LocalTtsState>,
    voice_id: String,
) -> Result<(), String> {
    model_manager::delete_voice(&state.paths, &voice_id)
}

#[tauri::command]
pub async fn tts_local_import_style_vectors(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    voice_id: String,
    path: String,
) -> Result<ImportResult, String> {
    if voice_id.is_empty() || voice_id.len() > 64 {
        return Err("voice id length out of range".into());
    }
    if !voice_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("voice id must be kebab-case ASCII".into());
    }

    let voice_dir = state.paths.voice_dir(&voice_id);
    if !voice_dir.exists() {
        return Err(format!(
            "voice {voice_id} not found; import the .onnx or .sbv2 model first"
        ));
    }
    if voice_dir.join("model.sbv2").exists() {
        return Err(format!(
            "voice {voice_id} is .sbv2 form; style vectors are embedded and cannot be replaced"
        ));
    }

    let (src, cleanup_after_import) =
        saf_bridge::prepare_file_import_source(&app, &path).await?;
    let result: std::result::Result<ImportResult, String> = async {
        if !src.exists() {
            return Err(format!("path not found: {path}"));
        }
        let destination = state.paths.style_vectors_path(&voice_id);
        std::fs::copy(&src, &destination)
            .map_err(|e| format!("copy style_vectors.json: {e}"))?;
        let bytes = std::fs::metadata(&destination).map(|m| m.len()).unwrap_or(0);
        let _ = app.emit("tts://install-complete", &voice_id);
        Ok(ImportResult {
            asset_id: voice_id.clone(),
            voice_id: Some(voice_id),
            path: destination.to_string_lossy().into_owned(),
            bytes,
            message: "style vectors imported".into(),
        })
    }
    .await;

    if cleanup_after_import {
        let _ = tokio::fs::remove_file(&src).await;
    }
    result
}

#[tauri::command]
pub async fn tts_local_synthesize_preview(
    state: State<'_, LocalTtsState>,
    text: String,
    voice_id: String,
    length_scale: f32,
    sdp_ratio: f32,
) -> Result<Response, String> {
    if !state.engine.is_ready().await {
        return Err(
            "local TTS engine not initialized (missing DeBerta)".into()
        );
    }
    state.engine.load_voice(&state.paths, &voice_id).await?;
    let req = SynthesizeRequest {
        voice_id,
        text,
        style_id: 0,
        speaker_id: 0,
        sdp_ratio,
        length_scale,
    };
    state.engine.synthesize(req).await.map(wav_response)
}

fn wav_response(bytes: Vec<u8>) -> Response {
    Response::new(bytes)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::ipc::{InvokeResponseBody, IpcResponse};

    fn test_paths(root: &std::path::Path) -> LocalTtsPaths {
        LocalTtsPaths {
            root: root.join("models").join("tts-local"),
            assets: root.join("models").join("tts-local").join("assets"),
            voices: root.join("models").join("tts-local").join("voices"),
            cache: root.join("cache"),
        }
    }

    #[test]
    fn local_tts_switch_can_be_changed_at_runtime() {
        let switch = LocalTtsSwitch::new(false);
        assert!(!switch.is_enabled());
        switch.set_enabled(true);
        assert!(switch.is_enabled());
        switch.set_enabled(false);
        assert!(!switch.is_enabled());
    }

    #[test]
    fn preview_wav_uses_raw_ipc_response() {
        let response = wav_response(vec![0x52, 0x49, 0x46, 0x46]);
        match response.body().unwrap() {
            InvokeResponseBody::Raw(bytes) => assert_eq!(bytes, b"RIFF"),
            InvokeResponseBody::Json(_) => panic!("preview WAV was JSON serialized"),
        }
    }

    #[test]
    fn shared_deberta_import_uses_expected_file_names() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let source = temp.path().join("downloaded.bin");
        std::fs::write(&source, b"fixture").unwrap();

        let model = install_shared_asset(&paths, &source, "deberta").unwrap();
        assert_eq!(model, paths.deberta_dir().join("deberta.onnx"));
        assert_eq!(std::fs::read(model).unwrap(), b"fixture");

        let tokenizer =
            install_shared_asset(&paths, &source, "deberta-tokenizer").unwrap();
        assert_eq!(tokenizer, paths.deberta_dir().join("tokenizer.json"));
        assert_eq!(std::fs::read(tokenizer).unwrap(), b"fixture");
    }

    #[test]
    fn shared_asset_import_rejects_unknown_asset() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let source = temp.path().join("downloaded.bin");
        std::fs::write(&source, b"fixture").unwrap();

        let error = install_shared_asset(&paths, &source, "voice-model").unwrap_err();
        assert!(error.contains("unknown shared asset"));
    }

    #[test]
    fn shared_asset_download_uses_individual_canonical_file_names() {
        assert_eq!(shared_asset_file_name("deberta").unwrap(), "deberta.onnx");
        assert_eq!(
            shared_asset_file_name("deberta-tokenizer").unwrap(),
            "tokenizer.json"
        );
        assert!(shared_asset_file_name("unknown").is_err());
    }

    #[test]
    fn download_temp_path_preserves_catalog_extension() {
        let cache = Path::new("C:/tts-cache");
        let voice = registry::find("ling-v2").unwrap();
        let style = registry::find("ling-v2-style").unwrap();
        assert_eq!(
            download_temp_path(&voice, cache),
            PathBuf::from("C:/tts-cache/ling-v2.download.onnx")
        );
        assert_eq!(
            download_temp_path(&style, cache),
            PathBuf::from("C:/tts-cache/ling-v2-style.download.json")
        );
    }

    #[test]
    fn style_vectors_resolves_to_voice_directory() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let source = temp.path().join("downloaded.json");
        std::fs::write(&source, b"{\"v\":1}").unwrap();

        let installed = install_style_vectors_for(&paths, &source, "ling-v2").unwrap();
        let expected = paths.voice_dir("ling-v2").join("style_vectors.json");
        assert_eq!(installed, expected);
        assert_eq!(std::fs::read(installed).unwrap(), b"{\"v\":1}");
    }
}
