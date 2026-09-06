pub mod role_sync;
pub mod static_copy;
pub mod voice_cleanup;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use tauri::App;
use tauri::Emitter;
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

use crate::ChatComponents;
use crate::ai_service::emotion::EmotionClassifier;
use crate::ai_service::game_system::persistent_memory_system::MemorySectionLimits;
use crate::ai_service::llm::LlmSlot;
use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, migrate_if_needed, migrate_legacy_vision_keys,
    resolve_chat_provider, resolve_translate_provider,
};
use crate::ai_service::message_system::processor::{MessageProcessor, ProcessorOptions};
use crate::ai_service::service::{AIService, SharedAIService};
use crate::ai_service::translator::Translator;
use crate::ai_service::tts::local::LocalTtsRuntime;
use crate::ai_service::types::CharacterSettings;
use crate::config::{self, AppConfig};
use crate::db;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;

pub async fn initialize(
    app: &App,
    local_tts: Option<LocalTtsRuntime>,
) -> Result<(DatabaseConnection, SharedAIService, ChatComponents)> {
    // init_data_dir 已经在 Tauri 设置闭包中提前调用过了
    // （参见 lib.rs），因此在此函数运行之前，缓存的数据目录就已经对
    // LocalTtsPaths::resolve 可用了。如果在这里再次调用它，会导致
    // OnceLock 发生 panic。
    static_copy::seed_data_dir(&app.handle())?;
    let data_dir = static_copy::get_data_dir().clone();

    // 应用 LAN 同步暂存文件（必须在 DB 初始化之前，否则 .db 仍被锁定）
    crate::lan_sync::staging::apply_staged_files(&data_dir);

    let db = db::init_db(&data_dir).await?;

    // 导入 LAN 同步暂存的数据库记录（表结构就绪后才执行）
    let db_imported = crate::lan_sync::db_sync::apply_staged_db_records(&db, &data_dir)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    if db_imported > 0 {
        tracing::info!("已导入 {} 条数据库记录（来自 LAN 同步）", db_imported);
    }

    role_sync::sync_roles_from_folder(&db, &data_dir).await?;

    // 确保玩家 User 角色存在（id=0，用于 line.sender_role_id 的 FK 约束）
    RoleRepo::ensure_user_role(&db).await?;

    // 迁移旧的扁平 LLM 配置 → 多供应商列表
    migrate_if_needed(&app.handle());
    // 迁移旧的主动视觉独立配置（VD_*）→ 大模型管理中的视觉模型角色
    migrate_legacy_vision_keys(&app.handle());

    // 提前加载配置 + 构建 LlmClient（AIService 的子成员 GameRoleManager 需要它）
    let app_config = AppConfig::load(&app.handle()).unwrap_or_default();
    tracing::info!(
        "MemoryBank 配置: enabled={}, update_interval={}, recent_window={}, limits=[{},{},{},{}]（记忆设置需重启生效）",
        app_config.use_persistent_memory,
        app_config.memory_update_interval,
        app_config.memory_recent_window,
        app_config.memory_short_term_max_chars,
        app_config.memory_long_term_max_chars,
        app_config.memory_user_info_max_chars,
        app_config.memory_promises_max_chars,
    );

    // 构建聊天主 LLM 槽位（支持运行时热切换）。
    // 槽位本身始终存在，未配置模型时内部值为 None。
    let llm: LlmSlot = std::sync::Arc::new(tokio::sync::RwLock::new(
        resolve_chat_provider(&app.handle())
            .and_then(|p| build_llm_client_from_provider(&app.handle(), &p))
            .map(Arc::new),
    ));

    // AIService 内部的 GameRoleManager 共享同一个聊天 LLM 槽位
    let mut ai_service = AIService::new(
        db.clone(),
        data_dir.clone(),
        llm.clone(),
        app_config.tts.clone(),
        local_tts,
        app_config.use_persistent_memory,
        app_config.memory_update_interval,
        app_config.memory_recent_window,
        MemorySectionLimits {
            short_term: app_config.memory_short_term_max_chars as usize,
            long_term: app_config.memory_long_term_max_chars as usize,
            user_info: app_config.memory_user_info_max_chars as usize,
            promises: app_config.memory_promises_max_chars as usize,
        },
    )
    .await;

    // 加载默认角色：上次游玩的角色 → DB 中第一个主角色 → 默认空设定
    let settings = load_default_character(app, &db, &data_dir).await?;
    let character_id = settings.character_id;
    let prompt_options = PromptOptions {
        output_sec_lang: app_config.llm_output_sec_lang,
        no_emotion_limit: app_config.no_emotion_limit_prompt,
    };

    // 从 session store 读取各角色的上次服装，注入 GameRoleManager
    {
        let mut overrides = HashMap::new();
        if let Ok(store) = app.store(config::STORE_FILE) {
            if let Some(cid) = character_id {
                let key = config::session::last_clothes_key(cid);
                if let Some(clothes) = store.get(&key).and_then(|v| v.as_str().map(String::from)) {
                    if !clothes.is_empty() {
                        overrides.insert(cid, clothes);
                    }
                }
            }
        }
        ai_service.set_clothes_overrides(overrides).await;
    }

    ai_service
        .init_game_status(character_id, prompt_options)
        .await?;

    tracing::info!("AIService 初始化完成");

    let ai_service: SharedAIService = Arc::new(Mutex::new(ai_service));

    // —— 构建聊天组件 ——
    // 翻译 LLM 槽位（支持运行时热切换）；槽位本身始终存在。
    let translate_llm: LlmSlot = std::sync::Arc::new(tokio::sync::RwLock::new(
        resolve_translate_provider(&app.handle())
            .and_then(|p| build_llm_client_from_provider(&app.handle(), &p))
            .map(Arc::new),
    ));

    let classifier = load_emotion_classifier(app_config.enable_emotion_classifier, &data_dir);
    let processor = Arc::new(MessageProcessor::new(
        ProcessorOptions {
            time_sense_enabled: app_config.enable_time_sense,
            enable_translate: app_config.enable_translate,
        },
        classifier,
    ));

    let translator = Arc::new(Translator::new(
        translate_llm,
        !app_config.llm_output_sec_lang,
    ));

    let chat = ChatComponents {
        llm,
        processor,
        translator,
    };

    Ok((db, ai_service, chat))
}

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

fn load_emotion_classifier(
    enabled: bool,
    data_dir: &std::path::Path,
) -> Option<Arc<EmotionClassifier>> {
    if !enabled {
        tracing::info!("情绪分类器已在配置中禁用");
        return None;
    }

    let model_dir = resolve_emotion_model_dir(data_dir);
    match model_dir {
        Some(dir) if dir.join("model.onnx").exists() => match EmotionClassifier::load(&dir) {
            Ok(clf) => {
                tracing::info!("情绪分类器加载成功: {}", dir.display());
                return Some(Arc::new(clf));
            },
            Err(e) => {
                tracing::warn!(
                    "情绪分类器加载失败 ({}), 回退为禁用状态: {e}",
                    dir.display()
                );
            },
        },
        _ => {
            tracing::warn!("未找到情绪模型目录, 情绪分类器将禁用");
        },
    }

    None
}

fn resolve_emotion_model_dir(data_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    // 发布模式：data/emotion_model_19emo/
    let data_path = data_dir.join("third_party").join("emotion_model_19emo");
    if data_path.exists() {
        return Some(data_path);
    }

    None
}

/// 加载默认角色设定：上次游玩的角色 → 第一个主角色 → 默认空设定
async fn load_default_character(
    app: &App,
    db: &DatabaseConnection,
    data_dir: &std::path::Path,
) -> Result<CharacterSettings> {
    // 1. 尝试从 settings store 读取上次游玩的角色 ID
    let store = app
        .store(config::STORE_FILE)
        .unwrap_or_else(|_| app.handle().store(config::STORE_FILE).unwrap());
    if let Some(last_id) = store
        .get(config::session::LAST_CHARACTER_ID)
        .and_then(|v| v.as_i64())
    {
        if let Ok(Some(settings)) =
            RoleRepo::get_role_settings_by_id(db, data_dir, last_id as i32).await
        {
            tracing::info!("加载上次游玩的角色: id={}", last_id);
            return Ok(settings);
        }
    }

    // 2. 回退：取第一个主角色
    if let Ok(main_roles) = RoleRepo::get_all_main_roles(db).await {
        if let Some(role) = main_roles.first() {
            let folder = role.resource_folder.clone().unwrap_or_default();
            if let Ok(Some(settings)) =
                RoleRepo::get_role_settings_by_id(db, data_dir, role.id).await
            {
                tracing::info!("加载默认主角色: id={}, folder={}", role.id, folder);
                return Ok(settings);
            }
        }
    }

    // 3. 无角色可用时返回默认空设定
    tracing::warn!("无可用角色，使用默认空设定");
    Ok(CharacterSettings::default())
}
