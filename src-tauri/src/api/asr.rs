//! ASR 相关 Tauri commands。
//!
//! 10 个 command：
//! - `asr_start_listening` / `asr_stop_listening`：会话生命周期
//! - `asr_vad_process_chunk`：转发 PCM 块到 VAD
//! - `asr_recognize_wav`：单次识别（前端 mic 模式主动调用）
//! - `asr_cancel`：取消长生命周期任务
//! - `asr_list_providers`：列出所有 provider 元数据
//! - `asr_get_settings` / `asr_set_settings`：配置读写（set 会重建 registry）
//! - `asr_test_provider`：用 1 秒静音 WAV 探测 provider 可达性
//! - `asr_get_status`：查询运行时状态（VAD 是否就绪）

use tauri::AppHandle;
use tauri::Emitter;

use crate::AppState;
use crate::ai_service::asr::debug_log;
use crate::ai_service::asr::error::AsrError;
#[cfg(desktop)]
use crate::ai_service::asr::global_hotkey;
use crate::ai_service::asr::provider::{
    self, AsrOptions, AsrResult, Hotword, ProviderInfo, list_provider_info,
};
use crate::ai_service::asr::session::{AsrSession, AsrSource};
use crate::ai_service::asr::settings::{self, AsrSettings};

fn parse_source(s: &str) -> Result<AsrSource, String> {
    AsrSource::from_str(s).ok_or_else(|| format!("invalid source: {s}"))
}

/// 命令层的热词入参，兼容两种 JSON 形态：
///
/// ```json
/// ["量子计算", "Anthropic"]                    // 纯词表，用默认权重
/// [{"text": "量子计算", "weight": 5}]          // 带权重
/// ```
///
/// **这是热词的唯一入口**：ASR 设置页已不再有热词设置，热词归角色
/// （每个角色独立、存数据库），接入时只需在调用处填这个参数，
/// **provider 侧零改动**。前端目前一律传 `None`。
#[derive(serde::Deserialize)]
#[serde(untagged)]
pub enum HotwordInput {
    Text(String),
    Weighted { text: String, weight: Option<u8> },
}

impl HotwordInput {
    fn into_hotword(self) -> Hotword {
        match self {
            Self::Text(t) => Hotword::new(t),
            Self::Weighted { text, weight } => match weight {
                Some(w) => Hotword::with_weight(text, w),
                None => Hotword::new(text),
            },
        }
    }
}

/// 组装一次识别调用的参数。空词条（含只输空白的）直接丢弃。
fn build_options(language_hint: Option<String>, hotwords: Option<Vec<HotwordInput>>) -> AsrOptions {
    AsrOptions {
        language_hint,
        hotwords: hotwords
            .unwrap_or_default()
            .into_iter()
            .map(HotwordInput::into_hotword)
            .filter(|h| !h.text.trim().is_empty())
            .collect(),
    }
}

/// 错误转前端可读字符串：`{"code":"<i18n_code>","detail":"<详情>"}` JSON。
/// 前端统一用 `utils/asrError.ts` 的 parseAsrError 解析（JSON.parse 失败回退
/// 旧 `CODE|detail` 格式与原文）——ProviderApiError 的 detail 随 code 走，
/// 用户能看到具体失败原因而非笼统 code。
///
/// 所有 ASR command 的 `AsrError` 出口统一走此函数（不再直接回裸 i18n_code），
/// 保证前后端错误契约一致；仅 `session_ref` / `parse_source` 这类非 AsrError
/// 的基础设施错误仍以原文返回，前端按原文展示。
fn err_to_user(e: &AsrError) -> String {
    let code = e.i18n_code();
    let detail = match e {
        AsrError::ProviderApiError { message, .. } => Some(message.clone()),
        _ => None,
    };
    serde_json::json!({ "code": code, "detail": detail }).to_string()
}

/// 取 session Arc 引用：锁内只 clone Arc（微秒级），锁外调用长耗时方法——
/// 避免 asr_stop_streaming 等 30s 等待期间阻塞其它 ASR 命令
///（asr_recognize_wav 等已在网络调用前释放外层锁，此 helper 统一该模式）。
async fn session_ref(
    session_arc: &std::sync::Arc<tokio::sync::Mutex<Option<std::sync::Arc<AsrSession>>>>,
) -> Result<std::sync::Arc<AsrSession>, String> {
    let guard = session_arc.lock().await;
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| "ASR not initialized".to_string())
}

/// 新建带 30s 超时的 HTTP 客户端（provider 网络请求统一用它）。
fn build_http() -> Result<reqwest::Client, AsrError> {
    // TLS 必须走统一的 webpki-roots 配置：reqwest 默认的 rustls-platform-verifier
    // 在 Android 上未显式初始化会 panic（见 utils/tls.rs）。
    let tls_config = crate::utils::tls::build_tls_config()
        .map_err(|e| AsrError::EngineLoadFailed(format!("build tls config: {e}")))?;
    reqwest::Client::builder()
        .tls_backend_preconfigured(tls_config)
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
    let session = session_ref(&state.asr_state.session).await?;
    session.start(source).await.map_err(|e| err_to_user(&e))
}

#[tauri::command]
pub async fn asr_stop_listening(
    source: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let source = parse_source(&source)?;
    let Ok(session) = session_ref(&state.asr_state.session).await else {
        return Ok(()); // 未初始化视为幂等停止
    };
    session.stop(source).await.map_err(|e| err_to_user(&e))
}

#[tauri::command]
pub async fn asr_vad_process_chunk(
    app: AppHandle,
    pcm: Vec<f32>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let Ok(session) = session_ref(&state.asr_state.session).await else {
        // 诊断：session 未初始化（VAD 模型加载失败等）时静默丢块会掩盖故障
        tracing::warn!(
            "[ASR/VAD] session 未初始化，丢弃 chunk ({} samples)",
            pcm.len()
        );
        return Ok(());
    };
    session
        .vad_process_chunk(&app, pcm)
        .await
        .map_err(|e| err_to_user(&e))
}

#[tauri::command]
pub async fn asr_recognize_wav(
    provider_id: String,
    wav_bytes: Vec<u8>,
    language_hint: Option<String>,
    hotwords: Option<Vec<HotwordInput>>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AsrResult, String> {
    let session = session_ref(&state.asr_state.session).await?;
    // cancel_token 锁内克隆立即释放（微秒级），网络调用（最长 30s）不占锁
    let cancel_child = session.cancel_token.lock().await.clone().child_token();
    let http = build_http().map_err(|e| err_to_user(&e))?;
    // providers 注册表锁内 clone（Arc 共享，廉价），锁外 resolve
    let providers = session.providers.lock().await.clone();
    let p = resolve_provider(&providers, &provider_id, &app, &http)
        .await
        .map_err(|e| err_to_user(&e))?;
    let opts = build_options(language_hint, hotwords);
    tracing::info!(
        "[ASR] 发送音频到 {provider_id}: {} bytes, {} 条热词",
        wav_bytes.len(),
        opts.hotwords.len()
    );
    let result = tokio::select! {
        result = p.recognize(wav_bytes, &opts) => result,
        _ = cancel_child.cancelled() => Err(AsrError::Canceled),
    };
    match result {
        Ok(r) => {
            tracing::info!("[ASR] {provider_id} 识别结果: {}", r.text);
            Ok(r)
        },
        Err(e) => {
            // 诊断：暴露 provider 失败的具体细节
            tracing::error!("[ASR] {provider_id} 识别失败: {e}");
            // err_to_user：i18n code + detail（前端 parseAsrError 解析展示）
            Err(err_to_user(&e))
        },
    }
}

/// 结果流式识别（llama-asr 的 SSE 路径）：整段 WAV 上传 → 增量 partial
/// （`asr://stream_partial` 事件）→ 返回 final。
///
/// 与 WS 会话流式（asr_start_streaming / asr_stop_streaming）独立；qwen
/// 调用会得到 StreamingNotSupported（trait 默认实现）。
#[tauri::command]
pub async fn asr_recognize_wav_stream(
    provider_id: String,
    wav_bytes: Vec<u8>,
    hotwords: Option<Vec<HotwordInput>>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AsrResult, String> {
    let session = session_ref(&state.asr_state.session).await?;
    // cancel_token 锁内克隆立即释放（微秒级），网络调用（最长 30s）不占锁
    let cancel_child = session.cancel_token.lock().await.clone().child_token();
    let http = build_http().map_err(|e| err_to_user(&e))?;
    // providers 注册表锁内 clone（Arc 共享，廉价），锁外 resolve
    let providers = session.providers.lock().await.clone();
    let p = resolve_provider(&providers, &provider_id, &app, &http)
        .await
        .map_err(|e| err_to_user(&e))?;
    tracing::info!(
        "[ASR] 流式识别发送音频到 {provider_id}: {} bytes",
        wav_bytes.len()
    );
    // partial 事件统一由命令层发射（provider 只回传文本，展示与识别解耦）
    let app_handle = app.clone();
    let on_partial: Option<std::sync::Arc<dyn Fn(&str) + Send + Sync>> =
        Some(std::sync::Arc::new(move |text: &str| {
            let _ = app_handle.emit("asr://stream_partial", text.to_string());
        }));
    let opts = build_options(None, hotwords);
    let result = tokio::select! {
        result = p.stream_recognize(wav_bytes, &opts, on_partial) => result,
        _ = cancel_child.cancelled() => Err(AsrError::Canceled),
    };
    match result {
        Ok(r) => {
            tracing::info!("[ASR] {provider_id} 流式识别结果: {}", r.text);
            Ok(r)
        },
        Err(e) => {
            tracing::error!("[ASR] {provider_id} 流式识别失败: {e}");
            // err_to_user：i18n code + detail（前端 parseAsrError 解析展示）
            Err(err_to_user(&e))
        },
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
    let session = session_ref(&state.asr_state.session).await?;
    session.cancel_stream().await;
    session.cancel().await;
    Ok(())
}

#[tauri::command]
pub async fn asr_start_streaming(
    provider_id: String,
    language_hint: Option<String>,
    hotwords: Option<Vec<HotwordInput>>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let session = session_ref(&state.asr_state.session).await?;
    // 仅支持流式的 provider 可启动（注册表锁内读，微秒级）
    let supports = session
        .providers
        .lock()
        .await
        .get(&provider_id)
        .map(|p| p.supports_streaming())
        .unwrap_or(false);
    if !supports {
        return Err(err_to_user(&AsrError::StreamingNotSupported(provider_id)));
    }
    let settings = settings::load(&app).map_err(|e| err_to_user(&e))?;
    let creds = settings
        .provider_configs
        .get(&provider_id)
        .cloned()
        .unwrap_or_default()
        .to_credentials();
    // 流式模型回退：配置为空、或配了非流式模型（实时端点不认识它们，
    // 会返回 400 url error）→ 该地域的流式默认模型。
    // **仅 qwen provider 参与回退**：其它 provider 的模型名不能套 qwen 的清单
    //（旧实现在这里写死 "paraformer-realtime-v2" 且不判断 provider，会把 qwen
    // 的模型名塞给 llama-asr 之类的 provider）。
    let model = if provider_id == provider::QwenAsrProvider::ID {
        let m = creds.model.trim();
        if m.is_empty() || !provider::qwen_is_streaming_model(m) {
            provider::qwen_default_model(true, creds.region_enum()).to_string()
        } else {
            m.to_string()
        }
    } else {
        creds.model.clone()
    };
    let opts = build_options(language_hint, hotwords);
    session
        .start_streaming(&app, &provider_id, &creds, &model, &opts)
        .await
        .map_err(|e| err_to_user(&e))
}

#[tauri::command]
pub async fn asr_stream_audio_chunk(
    pcm: Vec<f32>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let session = session_ref(&state.asr_state.session).await?;
    session
        .stream_audio_chunk(pcm)
        .await
        .map_err(|e| err_to_user(&e))
}

#[tauri::command]
pub async fn asr_stop_streaming(state: tauri::State<'_, AppState>) -> Result<AsrResult, String> {
    let session = session_ref(&state.asr_state.session).await?;
    session.stop_streaming().await.map_err(|e| err_to_user(&e))
}

/// 丢弃流式会话（异常路径清理用）：只 take 流式句柄、断开连接，
/// 不 cancel 非流式在飞识别（与 asr_cancel 的全局取消区分）。
#[tauri::command]
pub async fn asr_cancel_streaming(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let session = session_ref(&state.asr_state.session).await?;
    session.cancel_stream().await;
    Ok(())
}

#[tauri::command]
pub async fn asr_list_providers() -> Vec<ProviderInfo> {
    list_provider_info()
}

/// `region`：调用方当前选中的地域，优先于持久化配置（设置页改地域后要立刻刷新
/// 模型清单，而保存有 500ms debounce，见 `provider::list_models`）。缺省/空白
/// 时读持久化配置。
#[tauri::command]
pub async fn asr_list_models(
    provider_id: String,
    region: Option<String>,
    app: AppHandle,
) -> Result<Vec<provider::ModelInfo>, String> {
    // llama-asr 需要发 HTTP 请求拉服务端模型列表（qwen 是静态清单，不走网络）
    let http = build_http().map_err(|e| err_to_user(&e))?;
    provider::list_models(&provider_id, region.as_deref(), &app, &http)
        .await
        .map_err(|e| err_to_user(&e))
}

#[tauri::command]
pub async fn asr_get_settings(app: AppHandle) -> Result<AsrSettings, String> {
    settings::load(&app).map_err(|e| err_to_user(&e))
}

#[tauri::command]
pub async fn asr_set_settings(
    settings: AsrSettings,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    settings::save(&app, &settings).map_err(|e| err_to_user(&e))?;
    // 重建 provider registry（settings 改了 credentials 后立即生效）
    rebuild_providers(&state, &settings).await?;
    // 逐帧 VAD 调试日志开关立即生效。模块内原子量，不依赖 VAD session 是否就绪
    // （VAD 加载失败时也应记录设置值，故不等下面的 session_ref）
    debug_log::set(settings.vad_debug_log);
    // VAD 静音计时立即生效（下一轮录音按新配置切分）；
    // session 未初始化（VAD 加载失败）时跳过——设置本身已保存成功
    if let Ok(session) = session_ref(&state.asr_state.session).await {
        session
            .vad
            .set_silence_timeout_ms(settings.vad_silence_ms)
            .await;
    }
    // 全局快捷键同步（仅桌面）：开关开 → 注册 ptt_key 映射的组合串，关 → 注销。
    // 注册失败（键被占用/插件不支持）：保存本身已成功，**不返回 Err**——返回 Err
    // 会让前端 store 不提交（与落盘文件分叉），且错误可见性已由 emit 状态事件
    // 承担（设置页红字提示，审查中危 2）。
    //
    // 三种结果都上报（registered / inactive / failed）：前端既要据此提示用户，
    // 也要据此决定窗口内 keydown 是否退位（pttGlobalOk）。状态判别集中在
    // `global_hotkey::status_from` —— 此前三个分支各自手写 ok/reason，「已注销」
    // 与「注册失败」共用 ok:false，关闭开关时设置页误报「注册失败」。
    #[cfg(desktop)]
    {
        let result = global_hotkey::sync(&app, &settings);
        if let Err(e) = &result {
            tracing::warn!("[ASR] 全局快捷键注册失败: {e}");
        }
        let _ = app.emit_to(
            "main",
            "asr:ptt-global-status",
            global_hotkey::status_from(result, settings.ptt_global),
        );
    }
    Ok(())
}

#[tauri::command]
pub async fn asr_test_provider(
    provider_id: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let silence_wav = synth_silence_wav(1.0);
    let session = session_ref(&state.asr_state.session).await?;
    // cancel_token 锁内克隆立即释放（微秒级），网络请求（最长 30s）不占锁
    let cancel_child = session.cancel_token.lock().await.clone().child_token();
    let http = build_http().map_err(|e| err_to_user(&e))?;
    // providers 注册表锁内 clone（Arc 共享，廉价），锁外 resolve
    let providers = session.providers.lock().await.clone();
    let p = resolve_provider(&providers, &provider_id, &app, &http)
        .await
        .map_err(|e| err_to_user(&e))?;
    tracing::info!("[ASR] 测试连接: 发送静音探测到 {provider_id}");
    // 探测不带热词（只验连通性与 key 合法性）
    let opts = AsrOptions::default();
    let result = tokio::select! {
        result = p.recognize(silence_wav, &opts) => result,
        _ = cancel_child.cancelled() => Err(AsrError::Canceled),
    };
    match result {
        Ok(r) => {
            tracing::info!("[ASR] 测试连接 {provider_id} 成功: {}", r.text);
            Ok(())
        },
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
        },
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
    let http = build_http().map_err(|e| err_to_user(&e))?;
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
        },
        Err(e) => {
            tracing::warn!(
                "[ASR] rebuild provider {} failed ({}): {}",
                s.active_provider,
                e.i18n_code(),
                e
            );
        },
    }
    let session_arc = state.asr_state.session.clone();
    let guard = session_arc.lock().await;
    if let Some(session) = guard.as_ref() {
        *session.providers.lock().await = providers;
    }
    Ok(())
}

/// ASR 运行时状态（设置页状态面板）。
#[derive(serde::Serialize)]
pub struct AsrStatus {
    /// VAD 模型是否加载成功。session 存在 = init_asr 完成 = AsrVad::load 成功
    /// （模型加载失败会直接中断 init_asr，session 保持 None）。
    pub vad_loaded: bool,
    /// 全局快捷键注册是否健康（ptt_global=true 且实际已注册）。设置页启动查询用——
    /// asr:ptt-global-status 事件不缓存，打开晚于失败时刻会错过。
    pub ptt_global_ok: bool,
}

/// 查询 ASR 运行时状态。
///
/// 设置页状态面板用。`asr://vad_ready` 事件在启动早期 emit，前端监听器注册
/// 晚于事件会丢失（Tauri 事件不缓存历史）——查询式获取无竞态。
#[tauri::command]
pub async fn asr_get_status(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<AsrStatus, String> {
    let guard = state.asr_state.session.lock().await;
    // 全局快捷键健康检查（仅桌面）：开关开且实际注册成功才算 ok。
    // 注册失败（键被占用等）时设置页据此显示红字提示
    #[cfg(desktop)]
    let ptt_global_ok = settings::load(&app)
        .map(|s| global_hotkey::is_healthy(&app, &s))
        .unwrap_or(false);
    #[cfg(not(desktop))]
    let ptt_global_ok = false;
    Ok(AsrStatus {
        vad_loaded: guard.as_ref().is_some(),
        ptt_global_ok,
    })
}

/// 全局快捷键的界面门控（前端 chatActive 驱动）：仅 /chat 与 /pet 界面激活。
/// 离开界面注销释放键位（OS 级注册会拦截其它应用的同键输入）；
/// 回到界面按当前设置重新注册。移动端 no-op（全局快捷键为桌面端专属）。
#[tauri::command]
pub async fn asr_ptt_global_set_active(app: AppHandle, active: bool) -> Result<(), String> {
    #[cfg(desktop)]
    {
        let settings = settings::load(&app).map_err(|e| err_to_user(&e))?;
        let result = global_hotkey::set_active(&app, active, &settings);
        // 门控开启时才上报状态：前端 runtime.pttGlobalOk 必须与实际注册同步
        //（true 才让窗口内 keydown 退位；注册失败必须回到 false 由窗口内兜底，
        // 否则双源触发 toggle）。门控关闭（设置页打开）不上报——设置页监听
        // 该事件提示失败，门控关闭时"未注册"是预期状态，上报会造成误报。
        if active {
            // 与 asr_set_settings 共用同一套状态判别（见 status_from 的说明）
            let status = global_hotkey::status_from(result.clone(), settings.ptt_global);
            let _ = app.emit_to("main", "asr:ptt-global-status", status);
        }
        result
    }
    #[cfg(not(desktop))]
    {
        let _ = (app, active);
        Ok(())
    }
}
