//! 数据层引导。
//!
//! 原 `init/mod.rs::initialize()` 的前半段：播种数据目录 → 应用 LAN 同步暂存 →
//! 打开数据库 → 导入 LAN 暂存记录 → 同步角色 → 建表迁移 → 加载 `AppConfig`。
//! 后半段（构建 AIService 与服务图）已并入 [`super::build`]。
//!
//! 分界线是 `db`：本函数产出 `db` 后交回编排层，编排层要先用 `&db` 跑一次
//! 孤儿语音清理，再把 `db` 交给建图（见 [`super::setup`]）。

use anyhow::Result;
use sea_orm::DatabaseConnection;

use crate::ai_service::llm::provider_config::{migrate_if_needed, migrate_legacy_vision_keys};
use crate::config::AppConfig;
use crate::db;
use crate::db::managers::role_repo::RoleRepo;

/// 引导数据层，返回 `(db, app_config)`。
pub async fn bootstrap(app: &tauri::App<tauri::Wry>) -> Result<(DatabaseConnection, AppConfig)> {
    // init_data_dir 已经在 Tauri 设置闭包中提前调用过了
    // （参见 lib.rs），因此在此函数运行之前，缓存的数据目录就已经对
    // LocalTtsPaths::resolve 可用了。如果在这里再次调用它，会导致
    // OnceLock 发生 panic。
    crate::data_dir::seed_data_dir(&app.handle())?;
    let data_dir = crate::data_dir::get_data_dir().clone();

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

    crate::db::role_sync::sync_roles_from_folder(&db, &data_dir).await?;

    // 确保玩家 User 角色存在（id=0，用于 line.sender_role_id 的 FK 约束）
    RoleRepo::ensure_user_role(&db).await?;

    // 迁移旧的扁平 LLM 配置 → 多供应商列表
    migrate_if_needed(&app.handle());
    // 迁移旧的主动视觉独立配置（VD_*）→ 大模型管理中的视觉模型角色
    migrate_legacy_vision_keys(&app.handle());

    // 提前加载配置 + 构建 LlmClient（AIService 的子成员 GameRoleManager 需要它）
    let app_config = AppConfig::load(&app.handle()).unwrap_or_default();
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
