//! ASR 服务初始化。
//!
//! 原先位于 `init/mod.rs`；它只被 `app::setup::background` 调用一次，属启动装配的一部分。

use std::sync::Arc;

use anyhow::Result;
use tauri::Emitter;

/// ASR 服务初始化：加载 VAD 模型 + 构建 provider registry + 写入 AsrState。
///
/// 失败返回 Err，由调用方决定是否降级（v1:失败 → ASR 不可用但不阻塞主程序）。
///
/// 调用方需保证传入的 `asr_state` 是已经 manage 进 AppState 的那个 Arc；
/// 本函数只 mutate 内部的 `session: Option<AsrSession>`，不会重建外层 Arc。
pub async fn init_asr(
    app: &tauri::AppHandle,
    asr_state: &Arc<crate::ai_service::asr::AsrState>,
) -> Result<(), Box<dyn std::error::Error>> {
    use crate::ai_service::asr::{provider, session::AsrSession, settings, vad::AsrVad};

    tracing::info!("[ASR] init_asr 开始");
    let cfg = settings::load(app)?;
    // TLS 走统一的 webpki-roots 配置（Android 上 rustls-platform-verifier 未初始化会 panic）
    let tls_config = crate::utils::tls::build_tls_config()?;
    let http = reqwest::Client::builder()
        .tls_backend_preconfigured(tls_config)
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let mut providers: std::collections::HashMap<
        String,
        std::sync::Arc<dyn provider::AsrProvider>,
    > = std::collections::HashMap::new();
    // 只构建 active_provider：用户选哪个 STT 就启用哪个，未选的不初始化、
    // 不报错（日志干净，registry 只含当前服务商）。
    let cred = cfg
        .provider_configs
        .get(&cfg.active_provider)
        .map(|c| c.to_credentials())
        .unwrap_or_default();
    match provider::get_provider(&cfg.active_provider, &cred, &http).await {
        Ok(p) => {
            providers.insert(cfg.active_provider.clone(), p);
            tracing::info!("[ASR] provider {} 已构建", cfg.active_provider);
        },
        Err(e) => {
            tracing::warn!(
                "[ASR] provider {} 构建失败: {}",
                cfg.active_provider,
                e.i18n_code()
            );
        },
    }

    let vad = AsrVad::load(app)?;
    // 应用持久化的 VAD 静音计时（设置页可自定义，默认 800ms）
    vad.set_silence_timeout_ms(cfg.vad_silence_ms).await;
    let session = Arc::new(AsrSession::new(Arc::new(vad), providers));
    *asr_state.session.lock().await = Some(session);

    // 通知前端 VAD 模型就绪（设置页状态面板显示"已加载"）
    let _ = app.emit("asr://vad_ready", ());

    tracing::info!("[ASR] init_asr 完成");
    Ok(())
}
