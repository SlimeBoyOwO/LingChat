// Tauri commands exposing the local TTS engine to the frontend.

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
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
    Ok(TtsLocalStatus {
        ready: state.engine.is_ready().await,
        deberta_installed: state.paths.asset_present("deberta"),
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

fn default_voice_id(
    inspected: &archive::InspectedPackage,
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
) -> Result<ImportResult, String> {
    let src = PathBuf::from(&path);
    if !src.exists() {
        return Err(format!("path not found: {path}"));
    }
    let inspected = archive::inspect_package(&src)?;
    let voice_id = match voice_id {
        Some(v) => v,
        None => default_voice_id(&inspected, &src),
    };
    let installed =
        archive::install_inspected(&inspected, &src, &state.paths, &voice_id)?;
    let bytes = std::fs::metadata(&installed).map(|m| m.len()).unwrap_or(0);
    let _ = app.emit("tts://install-complete", &voice_id);
    Ok(ImportResult {
        asset_id: voice_id.clone(),
        voice_id: Some(voice_id),
        path: installed.to_string_lossy().into_owned(),
        bytes,
        message: "imported".into(),
    })
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
                let dst = state.paths.deberta_dir().join("deberta.onnx");
                let tok_dst = state.paths.deberta_dir().join("tokenizer.json");
                std::fs::create_dir_all(state.paths.deberta_dir())
                    .map_err(|e| format!("mkdir deberta: {e}"))?;
                let bytes =
                    download::download_asset(&app, &entry, &dst, cancel.clone())
                        .await?;
                if !tok_dst.exists() {
                    let tok_entry = registry::find("deberta-tokenizer")
                        .ok_or_else(|| "tokenizer URL not in catalog".to_string())?;
                    let _ = download::download_asset(
                        &app,
                        &tok_entry,
                        &tok_dst,
                        cancel.clone(),
                    )
                    .await?;
                }
                Ok(ImportResult {
                    asset_id: entry.id.clone(),
                    voice_id: None,
                    path: dst.to_string_lossy().into_owned(),
                    bytes,
                    message: "deberta downloaded".into(),
                })
            }
            registry::AssetKind::Voice => {
                let raw_dst = state
                    .paths
                    .cache
                    .join(format!("{}.download", entry.id));
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

#[tauri::command]
pub async fn tts_local_synthesize_preview(
    state: State<'_, LocalTtsState>,
    text: String,
    voice_id: String,
    length_scale: f32,
    sdp_ratio: f32,
) -> Result<Vec<u8>, String> {
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
    state.engine.synthesize(req).await
}
