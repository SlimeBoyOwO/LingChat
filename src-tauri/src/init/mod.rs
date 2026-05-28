pub mod role_sync;
pub mod static_copy;

use std::sync::Arc;

use anyhow::Result;
use sea_orm::DatabaseConnection;
use tauri::App;
use tauri_plugin_store::StoreExt;
use tokio::sync::Mutex;

use crate::ai_service::emotion::EmotionClassifier;
use crate::ai_service::llm::provider_config::{
    build_llm_client_from_provider, migrate_if_needed, resolve_chat_provider,
    resolve_translate_provider,
};
use crate::ai_service::message_system::processor::{MessageProcessor, ProcessorOptions};
use crate::ai_service::service::{AIService, SharedAIService};
use crate::ai_service::translator::Translator;
use crate::ai_service::types::CharacterSettings;
use crate::config::{self, AppConfig};
use crate::db;
use crate::db::managers::role_repo::RoleRepo;
use crate::utils::prompt::PromptOptions;
use crate::ChatComponents;

pub async fn initialize(
    app: &App,
) -> Result<(DatabaseConnection, SharedAIService, ChatComponents)> {
    static_copy::copy_game_data(app)?;

    let data_dir = static_copy::resolve_data_dir();
    let db = db::init_db(&data_dir).await?;

    role_sync::sync_roles_from_folder(&db, &data_dir).await?;

    // 迁移旧的扁平 LLM 配置 → 多供应商列表
    migrate_if_needed(&app.handle());

    // 提前加载配置 + 构建 LlmClient（AIService 的子成员 GameRoleManager 需要它）
    let app_config = AppConfig::load(&app.handle()).unwrap_or_default();

    let llm = resolve_chat_provider(&app.handle())
        .and_then(|p| build_llm_client_from_provider(&p))
        .map(Arc::new);

    let mut ai_service = AIService::new(
        db.clone(),
        data_dir.clone(),
        llm.clone(),
        app_config.tts.clone(),
    )
    .await;

    // 加载默认角色：上次游玩的角色 → DB 中第一个主角色 → 默认空设定
    let settings = load_default_character(app, &db, &data_dir).await?;
    let prompt_options = PromptOptions {
        output_sec_lang: app_config.llm_output_sec_lang,
        no_emotion_limit: app_config.no_emotion_limit_prompt,
    };
    ai_service.import_settings(settings, prompt_options).await;
    ai_service.init_game_status().await?;

    tracing::info!(
        "AIService 初始化完成: character_id={:?}, ai_name={}",
        ai_service.character_id,
        ai_service.ai_name,
    );

    let ai_service: SharedAIService = Arc::new(Mutex::new(ai_service));

    // —— 构建聊天组件 ——
    let translate_llm = resolve_translate_provider(&app.handle())
        .and_then(|p| build_llm_client_from_provider(&p));

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
            }
            Err(e) => {
                tracing::warn!(
                    "情绪分类器加载失败 ({}), 回退为禁用状态: {e}",
                    dir.display()
                );
            }
        },
        _ => {
            tracing::warn!("未找到情绪模型目录, 情绪分类器将禁用");
        }
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
        .get(config::keys::LAST_CHARACTER_ID)
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
