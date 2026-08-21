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
use crate::ai_service::asr::provider::{self, list_provider_info, AsrResult, ProviderInfo};
use crate::ai_service::asr::session::AsrSource;
use crate::ai_service::asr::settings::{self, AsrSettings};
use crate::AppState;

fn parse_source(s: &str) -> Result<AsrSource, String> {
    AsrSource::from_str(s).ok_or_else(|| format!("invalid source: {s}"))
}

/// 错误转前端可读字符串：i18n code（ProviderApiError 额外携带详情，格式 `CODE|detail`）。
/// 前端 SettingsAsr.testConnection 拆分后展示。
fn err_to_user(e: &AsrError) -> String {
    match e {
        AsrError::ProviderApiError { message, .. } => {
            format!("ASR_PROVIDER_FAILED|{message}")
        }
        _ => e.i18n_code().to_string(),
    }
}

/// 新建带 30s 超时的 HTTP 客户端（provider 网络请求统一用它）。
fn build_http() -> Result<reqwest::Client, AsrError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| AsrError::EngineLoadFailed(format!("build http client: {e}")))
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
        // 诊断：session 未初始化（VAD 模型加载失败等）时静默丢块会掩盖故障
        tracing::warn!(
            "[ASR/VAD] session 未初始化，丢弃 chunk ({} samples)",
            pcm.len()
        );
        Ok(())
    }
}

#[tauri::command]
pub async fn asr_recognize_wav(
    provider_id: String,
    wav_bytes: Vec<u8>,
    language_hint: Option<String>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AsrResult, String> {
    let session_arc = state.asr_state.session.clone();
    // 锁内只克隆 providers 注册表 + 取消令牌，立即释放锁：
    // 网络调用（最长 30s）不占用 session 锁，避免阻塞 asr_set_settings 等并发命令。
    let (providers, cancel_token) = {
        let guard = session_arc.lock().await;
        let s = guard.as_ref().ok_or("ASR not initialized")?;
        (s.providers.clone(), s.cancel_token.clone())
    };
    let http = build_http().map_err(|e| e.i18n_code().to_string())?;
    let p = resolve_provider(&providers, &provider_id, &app, &http)
        .await
        .map_err(|e| e.i18n_code().to_string())?;
    tracing::info!(
        "[ASR] 发送音频到 {provider_id}: {} bytes",
        wav_bytes.len()
    );
    let cancel_child = cancel_token.child_token();
    let result = tokio::select! {
        result = p.recognize(wav_bytes, language_hint.as_deref()) => result,
        _ = cancel_child.cancelled() => Err(AsrError::Canceled),
    };
    match result {
        Ok(r) => {
            tracing::info!("[ASR] {provider_id} 识别结果: {}", r.text);
            Ok(r)
        }
        Err(e) => {
            // 诊断：暴露 provider 失败的具体细节（之前仅前端 code，丢失 detail）
            tracing::error!("[ASR] {provider_id} 识别失败: {e}");
            Err(e.i18n_code().to_string())
        }
    }
}

/// 从 session registry 取 provider；不在 registry（如缺凭据被 init 跳过）时
/// 尝试用当前设置重建，从而把"缺 api_key"准确报告为 MissingCredentials，
/// 而不是误导性的 ProviderNotFound。
async fn resolve_provider(
    providers: &std::collections::HashMap<String, std::sync::Arc<dyn provider::AsrProvider>>,
    provider_id: &str,
    app: &AppHandle,
    http: &reqwest::Client,
) -> Result<std::sync::Arc<dyn provider::AsrProvider>, AsrError> {
    if let Some(p) = providers.get(provider_id) {
        return Ok(p.clone());
    }
    let settings = settings::load(app)?;
    let cred = settings
        .provider_configs
        .get(provider_id)
        .cloned()
        .unwrap_or_default();
    provider::get_provider(provider_id, &cred.to_credentials(), http)
        .await
        .map_err(|e| match e {
            AsrError::MissingCredentials(_) => e,
            _ => AsrError::ProviderNotFound(provider_id.into()),
        })
}

#[tauri::command]
pub async fn asr_cancel(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    if let Some(session) = guard.as_ref() {
        session.cancel_stream().await;
        session.cancel();
    }
    Ok(())
}

#[tauri::command]
pub async fn asr_start_streaming(
    provider_id: String,
    language_hint: Option<String>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    let session = guard.as_ref().ok_or("ASR not initialized")?;
    // 仅支持流式的 provider 可启动
    let supports = session
        .providers
        .get(&provider_id)
        .map(|p| p.supports_streaming())
        .unwrap_or(false);
    if !supports {
        return Err(AsrError::StreamingNotSupported(provider_id)
            .i18n_code()
            .to_string());
    }
    let settings = settings::load(&app).map_err(|e| e.i18n_code().to_string())?;
    let cred = settings
        .provider_configs
        .get(&provider_id)
        .cloned()
        .unwrap_or_default();
    // 流式模型：配置为空或为非流式模型（实时端点不认识 fun-asr-realtime，
    // 会返回 400 url error）→ 回退默认流式模型
    let model = if cred.model.is_empty() || !provider::qwen_is_streaming_model(&cred.model) {
        "paraformer-realtime-v2".to_string()
    } else {
        cred.model
    };
    session
        .start_streaming(&app, &provider_id, cred.api_key, model, language_hint)
        .await
        .map_err(|e| e.i18n_code().to_string())
}

#[tauri::command]
pub async fn asr_stream_audio_chunk(
    pcm: Vec<f32>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    let session = guard.as_ref().ok_or("ASR not initialized")?;
    session
        .stream_audio_chunk(pcm)
        .await
        .map_err(|e| e.i18n_code().to_string())
}

#[tauri::command]
pub async fn asr_stop_streaming(state: tauri::State<'_, AppState>) -> Result<AsrResult, String> {
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    let session = guard.as_ref().ok_or("ASR not initialized")?;
    session
        .stop_streaming()
        .await
        .map_err(|e| e.i18n_code().to_string())
}

#[tauri::command]
pub async fn asr_list_providers() -> Vec<ProviderInfo> {
    list_provider_info()
}

#[tauri::command]
pub async fn asr_list_models(provider_id: String) -> Vec<provider::ModelInfo> {
    provider::list_models(&provider_id)
}

#[tauri::command]
pub async fn asr_register_hotkey(
    combo: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .asr_state
        .hotkey
        .register(&app, &combo)
        .await
        .map_err(|e| e.i18n_code().to_string())
}

#[tauri::command]
pub async fn asr_unregister_hotkey(state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .asr_state
        .hotkey
        .unregister()
        .await
        .map_err(|e| e.i18n_code().to_string())
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
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let silence_wav = synth_silence_wav(1.0);
    let session_arc = state.asr_state.session.clone();
    // 克隆 providers + cancel_token 后释放锁：测试网络请求（最长 30s）不阻塞其他命令
    let (providers, cancel_token) = {
        let guard = session_arc.lock().await;
        let s = guard.as_ref().ok_or("ASR not initialized")?;
        (s.providers.clone(), s.cancel_token.clone())
    };
    let http = build_http().map_err(|e| e.i18n_code().to_string())?;
    let p = resolve_provider(&providers, &provider_id, &app, &http)
        .await
        .map_err(|e| err_to_user(&e))?;
    tracing::info!("[ASR] 测试连接: 发送静音探测到 {provider_id}");
    let cancel_child = cancel_token.child_token();
    let result = tokio::select! {
        result = p.recognize(silence_wav, None) => result,
        _ = cancel_child.cancelled() => Err(AsrError::Canceled),
    };
    match result {
        Ok(r) => {
            tracing::info!("[ASR] 测试连接 {provider_id} 成功: {}", r.text);
            Ok(())
        }
        Err(e) => {
            // 测试音频是 1 秒静音：部分 ASR（如 DashScope Fun-ASR）对静音
            // 直接返回 "ASR_RESPONSE_HAVE_NO_WORDS"。服务能响应这个错误
            // 恰好证明 API 可达 + key 有效 → 视为连接成功。
            if let AsrError::ProviderApiError { message, .. } = &e {
                if message.contains("NO_WORDS") || message.contains("no_words") {
                    tracing::info!("[ASR] 测试连接 {provider_id} 成功（静音无词，服务正常）");
                    return Ok(());
                }
            }
            tracing::warn!("[ASR] 测试连接 {provider_id} 失败: {e}");
            Err(err_to_user(&e))
        }
    }
}

/// 重建 provider registry——settings 改了之后生效。
/// 只构建 active_provider（用户选哪个 STT 就启用哪个，其余不初始化、不报错）。
/// 未配置 key 时构建失败仅 warn（不阻塞保存）；使用/测试时由
/// resolve_provider 给出准确的 MissingCredentials。
async fn rebuild_providers(
    state: &tauri::State<'_, AppState>,
    s: &AsrSettings,
) -> Result<(), String> {
    let http = build_http().map_err(|e| e.i18n_code().to_string())?;
    let mut providers: std::collections::HashMap<
        String,
        std::sync::Arc<dyn provider::AsrProvider>,
    > = std::collections::HashMap::new();
    let cred = s
        .provider_configs
        .get(&s.active_provider)
        .cloned()
        .unwrap_or_default();
    match provider::get_provider(&s.active_provider, &cred.to_credentials(), &http).await {
        Ok(p) => {
            providers.insert(s.active_provider.clone(), p);
        }
        Err(e) => {
            tracing::warn!(
                "[ASR] rebuild provider {} failed ({}): {}",
                s.active_provider,
                e.i18n_code(),
                e
            );
        }
    }
    let session_arc = state.asr_state.session.clone();
    let mut guard = session_arc.lock().await;
    if let Some(session) = guard.as_mut() {
        session.providers = providers;
    }
    Ok(())
}
