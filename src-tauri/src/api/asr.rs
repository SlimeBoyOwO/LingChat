//! ASR 相关 Tauri commands。
//!
//! 9 个 command：
//! - `asr_start_listening` / `asr_stop_listening`：会话生命周期
//! - `asr_vad_process_chunk`：转发 PCM 块到 VAD
//! - `asr_recognize_wav`：单次识别（前端 mic 模式主动调用）
//! - `asr_cancel`：取消长生命周期任务
//! - `asr_list_providers`：列出所有 provider 元数据
//! - `asr_get_settings` / `asr_set_settings`：配置读写（set 会重建 registry）
//! - `asr_test_provider`：用 1 秒静音 WAV 探测 provider 可达性

use tauri::AppHandle;

use crate::ai_service::asr::error::AsrError;
use crate::ai_service::asr::provider::{list_provider_info, AsrResult, ProviderInfo};
use crate::ai_service::asr::session::AsrSource;
use crate::ai_service::asr::settings::{self, AsrSettings};
use crate::ai_service::asr::AsrState;
use crate::AppState;

fn parse_source(s: &str) -> Result<AsrSource, String> {
    AsrSource::from_str(s).ok_or_else(|| format!("invalid source: {s}"))
}

/// 合成 1 秒静音 WAV（16kHz mono PCM16），用于 asr_test_provider 验证 API 可达性。
///
/// 仅做"能连通 + key 合法"的探测；不发声也不会影响识别结果。
fn synth_silence_wav(seconds: f32) -> Vec<u8> {
    let sample_rate = 16000u32;
    let num_samples = (seconds * sample_rate as f32) as u32;
    let byte_rate = sample_rate * 2; // mono * 16-bit
    let data_size = num_samples * 2;
    let mut buf = Vec::with_capacity((44 + data_size) as usize);
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_size).to_le_bytes());
    buf.extend_from_slice(b"WAVE");
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes());
    buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // mono
    buf.extend_from_slice(&sample_rate.to_le_bytes());
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes()); // block align
    buf.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_size.to_le_bytes());
    buf.resize((44 + data_size) as usize, 0);
    buf
}

// ========== 9 个 Tauri commands ==========

#[tauri::command]
pub async fn asr_start_listening(
    source: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let source = parse_source(&source)?;
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    let session = guard.as_ref().ok_or("ASR not initialized")?;
    session
        .start(source)
        .await
        .map_err(|e| e.i18n_code().to_string())
}

#[tauri::command]
pub async fn asr_stop_listening(
    source: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let source = parse_source(&source)?;
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    if let Some(session) = guard.as_ref() {
        session
            .stop(source)
            .await
            .map_err(|e| e.i18n_code().to_string())
    } else {
        Ok(()) // 未初始化视为幂等停止
    }
}

#[tauri::command]
pub async fn asr_vad_process_chunk(
    app: AppHandle,
    pcm: Vec<f32>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    if let Some(session) = guard.as_ref() {
        session
            .vad_process_chunk(&app, pcm)
            .await
            .map_err(|e| e.i18n_code().to_string())
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn asr_recognize_wav(
    provider_id: String,
    wav_bytes: Vec<u8>,
    language_hint: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<AsrResult, String> {
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    let session = guard.as_ref().ok_or("ASR not initialized")?;
    session
        .recognize_wav(provider_id, wav_bytes, language_hint)
        .await
        .map_err(|e| e.i18n_code().to_string())
}

#[tauri::command]
pub async fn asr_cancel(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    if let Some(session) = guard.as_ref() {
        session.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn asr_list_providers() -> Vec<ProviderInfo> {
    list_provider_info()
}

#[tauri::command]
pub async fn asr_get_settings(app: AppHandle) -> Result<AsrSettings, String> {
    settings::load(&app).map_err(|e| e.i18n_code().to_string())
}

#[tauri::command]
pub async fn asr_set_settings(
    settings: AsrSettings,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    settings::save(&app, &settings).map_err(|e| e.i18n_code().to_string())?;
    // 重建 provider registry（settings 改了 credentials 后立即生效）
    rebuild_providers(&state, &settings).await?;
    Ok(())
}

#[tauri::command]
pub async fn asr_test_provider(
    provider_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let silence_wav = synth_silence_wav(1.0);
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    let session = guard.as_ref().ok_or("ASR not initialized")?;
    session
        .recognize_wav(provider_id, silence_wav, None)
        .await
        .map(|_| ())
        .map_err(|e| e.i18n_code().to_string())
}

/// 重建 provider registry——settings 改了之后生效。
async fn rebuild_providers(
    state: &tauri::State<'_, AppState>,
    s: &AsrSettings,
) -> Result<(), String> {
    use crate::ai_service::asr::provider;
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("build http client: {e}"))?;
    let mut providers: std::collections::HashMap<
        String,
        std::sync::Arc<dyn provider::AsrProvider>,
    > = std::collections::HashMap::new();
    for info in provider::list_provider_info() {
        let cred = s
            .provider_configs
            .get(info.id)
            .cloned()
            .unwrap_or_default();
        match provider::get_provider(info.id, &cred.to_credentials(), &http).await {
            Ok(p) => {
                providers.insert(info.id.to_string(), p);
            }
            Err(e) => {
                tracing::warn!("[ASR] rebuild provider {} failed: {}", info.id, e.i18n_code());
            }
        }
    }
    let session_arc = state.asr_state.session.clone();
    let mut guard = session_arc.lock().await;
    if let Some(session) = guard.as_mut() {
        session.providers = providers;
    }
    Ok(())
}

// ========== 类型 re-export 辅助 ==========

/// 让 `tauri::command` 宏能正确解析 `AsrError` 的 `Serialize` derive（仅占位 import，
/// 实际序列化由宏根据返回类型自动注入）。
#[allow(dead_code)]
fn _ensure_asr_error_serialize(e: &AsrError) -> String {
    e.i18n_code().to_string()
}

/// 让 `tauri::command` 宏能正确解析 `AsrState` 字段类型（仅占位 import）。
#[allow(dead_code)]
fn _ensure_asr_state_in_scope(_: &AsrState) {}