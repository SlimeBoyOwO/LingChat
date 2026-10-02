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
    asr_state: &Arc<ling_chat_main::ai_service::asr::AsrState>,
) -> Result<(), Box<dyn std::error::Error>> {
    use ling_chat_main::ai_service::asr::{debug_log, provider, session::AsrSession, settings, vad::AsrVad};

    tracing::info!("[ASR] init_asr 开始");
    let mut cfg = settings::load(app)?;
    // 一次性迁移：把引入地域 / 双端点之前的 qwen 老配置补齐（详见
    // settings::migrate_provider_cfg）。放在 load 之后、构建 provider 之前，
    // 保证本次启动就用迁移后的值。**有变更才落盘**——迁移函数对已迁移过的
    // 配置直接返回 false，不会每次启动都重写 settings.json。
    let mut migrated = false;
    for (id, c) in cfg.provider_configs.iter_mut() {
        if settings::migrate_provider_cfg(id, c) {
            migrated = true;
        }
    }
    if migrated {
        if let Err(e) = settings::save(app, &cfg) {
            // 落盘失败不阻塞启动：内存里的 cfg 已是迁移后的值，本次运行正常
            tracing::warn!("[ASR] 配置迁移落盘失败（本次仍按迁移后的值运行）: {e}");
        } else {
            tracing::info!("[ASR] 已迁移老配置（补齐地域与双端点）");
        }
    }
    // 逐帧 VAD 调试日志开关（默认关）。放在 VAD 加载之前：加载失败会提前 return，
    // 开关值也要按设置落定，不留半初始化状态
    debug_log::set(cfg.vad_debug_log);
    // TLS 走统一的 webpki-roots 配置（Android 上 rustls-platform-verifier 未初始化会 panic）
    let tls_config = ling_chat_main::utils::tls::build_tls_config()?;
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

    let vad = match AsrVad::load(app) {
        Ok(vad) => vad,
        Err(e) => {
            // dll 缺失/加载失败：降级（ASR 不可用），绝不让启动失败
            tracing::warn!("[ASR] VAD 加载失败，ASR 本次启动降级为不可用: {e}");
            return Ok(());
        },
    };
    // 应用持久化的 VAD 静音计时（设置页可自定义，默认 800ms）
    vad.set_silence_timeout_ms(cfg.vad_silence_ms).await;
    let session = Arc::new(AsrSession::new(Arc::new(vad), providers));
    *asr_state.session.lock().await = Some(session);

    // 通知前端 VAD 模型就绪（设置页状态面板显示"已加载"）
    let _ = app.emit("asr://vad_ready", ());

    tracing::info!("[ASR] init_asr 完成");
    Ok(())
}
