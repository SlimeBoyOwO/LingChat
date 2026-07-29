// Tauri commands exposing the local TTS engine to the frontend.

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;

use super::archive;
use super::download;
use super::engine::SynthesizeRequest;
use super::model_manager;
use super::paths::LocalTtsPaths;
use super::registry::{self, AssetEntry};
use super::engine::LocalTtsEngine;

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
pub async fn tts_local_list_catalog() -> Result<Vec<AssetEntry>, String> {
    Ok(registry::all_assets())
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

/// Copy a downloaded shared asset into its canonical location under
/// `assets/deberta/`. Used for the DeBERTa model and tokenizer so the local
/// TTS engine finds them via `LocalTtsPaths::deberta_dir()`. Returns the
/// destination path on success.
pub fn install_shared_asset(
    paths: &LocalTtsPaths,
    src: &Path,
    asset_id: &str,
) -> Result<PathBuf, String> {
    let (target, label) = match asset_id {
        "deberta" => (paths.deberta_dir().join("deberta.onnx"), "DeBERTa model"),
        "deberta-tokenizer" => (paths.deberta_dir().join("tokenizer.json"), "DeBERTa tokenizer"),
        other => return Err(format!("unknown shared asset: {other}")),
    };
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create dir {}: {e}", parent.display()))?;
    }
    std::fs::copy(src, &target)
        .map_err(|e| format!("copy {label}: {e}"))?;
    Ok(target)
}

/// Copy a downloaded `style_vectors.json` blob into the named voice's
/// directory. Returns the canonical destination path.
pub fn install_style_vectors_for(
    paths: &LocalTtsPaths,
    src: &Path,
    voice_id: &str,
) -> Result<PathBuf, String> {
    let target = paths.voice_dir(voice_id).join("style_vectors.json");
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create dir {}: {e}", parent.display()))?;
    }
    std::fs::copy(src, &target)
        .map_err(|e| format!("copy style_vectors.json: {e}"))?;
    Ok(target)
}

fn shared_asset_file_name(asset_id: &str) -> Result<&'static str, String> {
    match asset_id {
        "deberta" => Ok("deberta.onnx"),
        "deberta-tokenizer" => Ok("tokenizer.json"),
        other => Err(format!("unknown BERT asset: {other}")),
    }
}

fn download_temp_path(entry: &AssetEntry, cache: &Path) -> PathBuf {
    let ext = registry::expected_extension(entry);
    cache.join(format!("{}.download.{ext}", entry.id))
}

fn default_voice_id(
    _inspected: &archive::InspectedPackage,
    src: &std::path::Path,
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

#[tauri::command]
pub async fn tts_local_import_from_path(
    app: AppHandle,
    state: State<'_, LocalTtsState>,
    path: String,
    voice_id: Option<String>,
    asset_id: Option<String>,
) -> Result<ImportResult, String> {
    let (src, cleanup_after_import) =
        super::import_bridge::prepare_file_import_source(&app, &path).await?;

    let result: std::result::Result<ImportResult, String> = async {
        if let Some(asset_id) = asset_id {
            // Shared-asset route (DeBERTa model / tokenizer): bypasses the
            // voice pipeline and lands in `assets/deberta/`.
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

        // Voice route: existing pipeline with SAF staging applied upstream.
        if !src.exists() {
            return Err(format!("path not found: {}", src.display()));
        }
        let inspected = archive::inspect_package(&src)?;
        let voice_id = match voice_id {
            Some(v) => v,
            None => default_voice_id(&inspected, &src),
        };
        let installed =
            archive::install_inspected(&inspected, &src, &state.paths, &voice_id)?;
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
) -> Result<ImportResult, String> {
    let entry = registry::find(&asset_id)
        .ok_or_else(|| format!("asset {asset_id} not in catalog"))?;

    let cancel = Arc::new(CancellationToken::new());
    {
        let mut guard = state.cancel.lock().await;
        *guard = Some(cancel.clone());
    }

    let result: std::result::Result<ImportResult, String> = async {
        match entry.kind {
            registry::AssetKind::Bert => {
                let file_name = shared_asset_file_name(&entry.id)?;
                let dst = state.paths.deberta_dir().join(file_name);
                std::fs::create_dir_all(state.paths.deberta_dir())
                    .map_err(|e| format!("mkdir deberta: {e}"))?;
                let bytes =
                    download::download_asset(&app, &entry, &dst, cancel.clone())
                        .await?;
                Ok(ImportResult {
                    asset_id: entry.id.clone(),
                    voice_id: None,
                    path: dst.to_string_lossy().into_owned(),
                    bytes,
                    message: format!("{} downloaded", entry.id),
                })
            }
            registry::AssetKind::Voice => {
                let raw_dst = download_temp_path(&entry, &state.paths.cache);
                let bytes =
                    download::download_asset(&app, &entry, &raw_dst, cancel.clone())
                        .await?;
                let inspected = archive::inspect_package(&raw_dst)?;
                let installed = archive::install_inspected(
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
                let raw_dst = download_temp_path(&entry, &state.paths.cache);
                let bytes =
                    download::download_asset(&app, &entry, &raw_dst, cancel.clone())
                        .await?;
                let installed = install_style_vectors_for(
                    &state.paths,
                    &raw_dst,
                    &voice_id,
                )?;
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
    .await;

    {
        let mut guard = state.cancel.lock().await;
        *guard = None;
    }
    let _ = app.emit("tts://download-complete", &asset_id);
    result
}

#[tauri::command]
pub async fn tts_local_delete_voice(
    state: State<'_, LocalTtsState>,
    voice_id: String,
) -> Result<(), String> {
    model_manager::delete_voice(&state.paths, &voice_id)
}

/// Import a `style_vectors.json` file into an existing voice directory.
///
/// Required when the voice is `model.onnx` form (vs the all-in-one `.sbv2`
/// which embeds style vectors internally). The target voice directory must
/// already exist; we only copy the JSON file alongside the model.
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
    // Refuse to install style vectors for a voice that already has the
    // all-in-one form (.sbv2) - the embedded style vectors would be ignored.
    if voice_dir.join("model.sbv2").exists() {
        return Err(format!(
            "voice {voice_id} is .sbv2 form; style vectors are embedded and cannot be replaced"
        ));
    }

    let (src, cleanup_after_import) =
        super::import_bridge::prepare_file_import_source(&app, &path).await?;
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
