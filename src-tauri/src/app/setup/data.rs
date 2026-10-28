//! 数据层引导：播种数据目录、打开数据库、同步角色与身份、执行迁移并加载配置。

use anyhow::Result;
use sea_orm::DatabaseConnection;

use ling_chat_main::ai_service::llm::provider_config::{
    migrate_if_needed, migrate_legacy_vision_keys,
};
use ling_chat_main::config::AppConfig;
use ling_chat_main::db;
use ling_chat_main::db::managers::role_repo::RoleRepo;

/// 引导数据层，返回 `(db, app_config)`。
pub async fn bootstrap(app: &tauri::App<tauri::Wry>) -> Result<(DatabaseConnection, AppConfig)> {
    ling_chat_main::data_dir::seed_data_dir(app.handle())?;
    let data_dir = ling_chat_main::data_dir::get_data_dir().clone();

    // 应用 LAN 同步暂存文件（必须在 DB 初始化之前，否则 .db 仍被锁定）
    ling_chat_main::lan_sync::staging::apply_staged_files(&data_dir);

    let db = db::init_db(&data_dir).await?;

    // 导入 LAN 同步暂存的数据库记录（表结构就绪后才执行）
    let db_imported = ling_chat_main::lan_sync::db_sync::apply_staged_db_records(&db, &data_dir)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    if db_imported > 0 {
        tracing::info!("已导入 {} 条数据库记录（来自 LAN 同步）", db_imported);
    }

    ling_chat_main::db::role_sync::sync_roles_from_folder(&db, &data_dir).await?;
    ling_chat_main::api::character::editor::cleanup_stale_editor_data(&db).await;

    // 身份卡同步进 player_identity 索引表（文件是真源，本表只做索引）。
    ling_chat_main::db::role_sync::sync_player_identities(&db, &data_dir)
        .await
        .map_err(|e| anyhow::anyhow!("身份索引同步失败: {e}"))?;

    // 确保玩家 User 角色存在（id=0，用于 line.sender_role_id 的 FK 约束）
    RoleRepo::ensure_user_role(&db).await?;

    // 迁移旧的扁平 LLM 配置 → 多供应商列表
    migrate_if_needed(app.handle());
    // 迁移旧的主动视觉独立配置（VD_*）→ 大模型管理中的视觉模型角色
    migrate_legacy_vision_keys(app.handle());

    // 提前加载配置 + 构建 LlmClient（AIService 的子成员 GameRoleManager 需要它）
    let app_config = AppConfig::load(app.handle()).unwrap_or_default();
    tracing::info!(
        "MemoryBank 配置: enabled={}, update_interval={}, recent_window={}, inject_continue_user={}, limits=[{},{},{},{}]（记忆设置需重启生效）",
        app_config.use_persistent_memory,
        app_config.memory_update_interval,
        app_config.memory_recent_window,
        app_config.memory_inject_continue_user,
        app_config.memory_short_term_max_chars,
        app_config.memory_long_term_max_chars,
        app_config.memory_user_info_max_chars,
        app_config.memory_promises_max_chars,
    );

    Ok((db, app_config))
}
