//! 角色编辑器的独立语音试听，不写角色设置、不影响聊天 Provider 的失败计数。
use std::io::Read;
use std::path::Path;

use tauri::{AppHandle, Manager, ipc::Response};

use crate::ai_service::{
    game_system::role_manager::build_voice_maker,
    tts::local::{LocalTtsRuntime, LocalTtsState, LocalTtsSwitch},
    types::CharacterSettings,
};
use crate::{AppState, config, db::managers::role_repo::RoleRepo};

const MAX_AUDIO_BYTES: usize = 20 * 1024 * 1024;
static PREVIEW_GATE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(1);

fn validate_preview_text(text: &str) -> Result<&str, String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > 500 {
        return Err("试听文本应为 1–500 个字符".into());
    }
    Ok(text)
}

fn read_reference_audio(path: &Path) -> Result<Vec<u8>, String> {
    let extension = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["wav", "mp3", "flac", "ogg", "m4a", "aac"].contains(&extension.as_str()) {
        return Err("请选择 WAV、MP3、FLAC、OGG、M4A 或 AAC 音频".into());
    }
    let file = std::fs::File::open(path).map_err(|e| format!("无法读取本机参考音频: {e}"))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_AUDIO_BYTES as u64 {
        return Err("参考音频必须是非空文件，且不超过 20 MB".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_AUDIO_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err("参考音频超过 20 MB".into());
    }
    Ok(bytes)
}

#[tauri::command]
pub async fn read_character_reference_audio(
    app: AppHandle,
    path: String,
) -> Result<Response, String> {
    let source =
        crate::ai_service::tts::local::saf_bridge::prepare_file_import_source(&app, &path).await?;
    let local_path = source.path.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || read_reference_audio(&local_path)).await;
    if source.cleanup_after_import {
        let _ = tokio::fs::remove_file(source.path).await;
    }
    result.map_err(|e| e.to_string())?.map(Response::new)
}

#[tauri::command]
pub async fn preview_character_voice(
    app: AppHandle,
    role_id: i32,
    settings: CharacterSettings,
    text: String,
    emotion: String,
) -> Result<Response, String> {
    let text = validate_preview_text(&text)?;
    if emotion.chars().count() > 32 {
        return Err("情绪名称过长".into());
    }
    let _permit = PREVIEW_GATE
        .try_acquire()
        .map_err(|_| "已有语音试听正在合成，请稍后重试")?;
    let state = app.state::<AppState>();
    RoleRepo::get_role_by_id(&state.db, role_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or("角色不存在")?;
    let store = config::settings_store(&app).map_err(|e| e.to_string())?;
    let tts_config = config::tts::TtsConfig::from_store(Some(&store));
    let local = app.try_state::<LocalTtsState>().and_then(|state| {
        app.try_state::<LocalTtsSwitch>().map(|switch| {
            LocalTtsRuntime::new(state.engine.clone(), state.paths.clone(), (*switch).clone())
        })
    });
    let voice = build_voice_maker(&super::data_dir(), &settings, &tts_config, local.as_ref())
        .ok_or("请先选择并配置角色的语音服务")?;
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        voice.synthesize_preview(text, &emotion),
    )
    .await
    .map_err(|_| "语音试听合成超时，请检查语音服务")?
    .map_err(|e| format!("语音试听失败: {e}"))?;
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
        return Err("语音服务返回空音频或超过 20 MB".into());
    }
    Ok(Response::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_text_validates_unicode_length() {
        assert_eq!(validate_preview_text(" 你好 ").unwrap(), "你好");
        assert!(validate_preview_text(" ").is_err());
        assert!(validate_preview_text(&"好".repeat(500)).is_ok());
        assert!(validate_preview_text(&"好".repeat(501)).is_err());
    }
    #[test]
    fn reference_audio_rejects_non_audio_empty_and_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reference.wav");
        std::fs::write(&path, b"RIFFtestWAVE").unwrap();
        assert_eq!(read_reference_audio(&path).unwrap(), b"RIFFtestWAVE");
        std::fs::write(&path, []).unwrap();
        assert!(read_reference_audio(&path).is_err());
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_AUDIO_BYTES as u64 + 1)
            .unwrap();
        assert!(read_reference_audio(&path).is_err());
        let path = dir.path().join("settings.yml");
        std::fs::write(&path, b"test").unwrap();
        assert!(read_reference_audio(&path).is_err());
    }
    #[tokio::test]
    async fn preview_uses_draft_voice_parameters_and_returns_audio_without_chat_files() {
        use axum::{Json, Router, routing::post};
        use serde_json::{Value, json};
        use std::sync::{Arc, Mutex};
        let received = Arc::new(Mutex::new(Value::Null));
        let capture = received.clone();
        let audio = b"RIFFtestWAVE".to_vec();
        let response = audio.clone();
        let router = Router::new().route(
            "/synthesize",
            post(move |Json(body): Json<Value>| {
                let capture = capture.clone();
                let response = response.clone();
                async move {
                    *capture.lock().unwrap() = body;
                    response
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        let config = config::tts::TtsConfig {
            sbv2api_api_url: format!("http://{address}"),
            ..Default::default()
        };
        let settings: CharacterSettings = serde_json::from_value(json!({
            "ai_name": "试听角色", "tts_type": "sbv2api", "voice_lang": "en",
            "voice_models": { "sbv2api_name": "draft-model", "sbv2api_speaker_id": "7" }
        }))
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let voice = build_voice_maker(dir.path(), &settings, &config, None).unwrap();
        let result = voice.synthesize_preview("Read this draft", "高兴").await;
        server.abort();
        assert_eq!(result.unwrap(), audio);
        let body = received.lock().unwrap();
        assert_eq!(body["text"], "Read this draft");
        assert_eq!(body["ident"], "draft-model");
        assert_eq!(body["speaker_id"], 7);
        assert!(!dir.path().join("voice").exists());
    }
}
