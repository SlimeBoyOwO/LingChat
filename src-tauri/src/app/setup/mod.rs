//! Tauri setup 阶段的启动编排。
//!
//! 顺序即语义，分四段：
//! 1. **建壳**（本文件）：注册日志桥与数据目录、`manage` 各小状态、`manage` 空壳
//!    `AppState`。空壳必须先于数据引导——Android 上 Tauri 在 setup 完成前就创建了
//!    webview，前端可能立刻 invoke 命令。
//! 2. **引导数据层**（[`data::bootstrap`]）：播种数据目录 → LAN 暂存 → 开库 →
//!    同步角色 → 迁移，产出 `db` 与 `AppConfig`。
//! 3. **建图**（[`build::build_service_graph`]）：构建全部领域服务，组装
//!    `InnerAppState`，再 `fill` 进 `AppState`。
//! 4. **起后台任务**（[`background::run`]）：依赖完整 `AppState` 的循环与处理器。
//!
//! `db` 在 2 与 3 之间回到本函数，是因为孤儿语音清理需要用 `&db`（且必须在 `db`
//! 被移入 `InnerAppState` 之前）。这个次序与重构前一致。
//!
//! 本模块原先内联在 `lib.rs` 的 `.setup()` 闭包中。错误类型统一为
//! [`Box<dyn std::error::Error>`]——与 `tauri::Builder::setup` 期望的完全一致，
//! 因此原先每一处 `?` 表达式都无需改写。

use crate::app::logging::LogFilterHandle;
use crate::app::state::AppState;
use crate::{ai_service, api, cast, data_dir, lan_sync, resource_sync, utils};
use tauri::Manager;

mod asr;
mod background;
mod build;
mod data;
mod voice_cleanup;

/// 执行 Tauri setup 阶段的全部初始化。
pub fn setup(
    app: &mut tauri::App<tauri::Wry>,
    log_filter: LogFilterHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    // 设置日志桥接的应用句柄
    utils::log_bridge::set_app_handle(app.handle().clone());

    // 提前初始化数据目录缓存，以便在数据层引导之前
    // 将其传递给独立的本地 TTS crate。
    data_dir::init_data_dir(&app.handle());

    // ONNX Runtime：定位 onnxruntime.dll 并显式加载
    // （仅 Windows 的 load-dynamic 模式，兼容无 AVX2 的旧 CPU，如三代酷睿；
    //  非 Windows 走 download-binaries 静态链接，无需此调用）。
    // 必须在任何 ort::Session 创建之前调用。
    #[cfg(target_os = "windows")]
    utils::onnx::init_onnx_runtime(app.handle());

    // 管理各种状态
    app.manage(api::pet::HitTestState::default());
    app.manage(resource_sync::ResourceSyncState::default());
    app.manage(lan_sync::LanSyncState::default());
    app.manage(cast::CastManager::default());
    app.manage(utils::cpu_perf::CpuDetectionCache::new());
    app.manage(utils::gpu_perf::GpuDetectionCache::new());
    app.manage(api::role_archive::RoleArchiveState::default());

    // Android 修复：Tauri 在 setup 闭包执行前已创建 webview 窗口，前端 invoke
    // 命令会在 IPC runtime worker 上立即 dispatch；如果 AppState 还没 manage
    // 就会 panic "state() called before manage()"。所以 setup 一开始就 manage
    // 一个空壳 AppState，引导完成后用真实值 fill。
    app.manage(AppState::empty());
    let rt = tokio::runtime::Runtime::new()?;
    // 本地 TTS（SBV2 进程内实现）：解析路径、注册 State/开关并收敛运行时。
    let local_tts = ai_service::tts::local::setup::bootstrap(app)?;
    let (db, app_config) = rt.block_on(data::bootstrap(app))?;

    // 初始化文件日志（从设置读取开关和保留天数）+ 应用 genai 调试开关
    crate::app::logging::apply_log_settings(app, &log_filter);
    // 热重载 genai 调试日志：settings store 变更时即时生效（无需重启）
    crate::app::logging::watch_genai_debug(app, log_filter);

    // 启动时自动清理未被引用的孤立语音文件
    match rt.block_on(voice_cleanup::cleanup_orphan_voice_files(&db, app.handle())) {
        Ok(stats) => {
            tracing::info!("语音文件清理完成: 删除 {} 个文件", stats.deleted_count);
        },
        Err(e) => {
            tracing::warn!("语音文件清理失败（非致命错误）: {e:#}");
        },
    }

    // 构建整个领域服务图并组装 InnerAppState。
    // 注意这里**不能**用 `rt.block_on` 包起来：建图过程中 `PluginManager` 会调用
    // `tokio::sync::Mutex::blocking_lock()`，在 runtime 内部阻塞会 panic。
    let build::Services {
        inner,
        auto_save_manager,
    } = build::build_service_graph(app, &rt, db, app_config, Some(local_tts.runtime.clone()))?;

    // 填充 AppState。块作用域必须保留：`State<'_>` 借用 `app`，
    // 而下面还要继续用 `app.handle()`。
    {
        let state = app.state::<AppState>();
        state.fill(inner);
    }

    // 依赖完整 AppState 的后台任务与处理器。
    background::run(app, &rt, &local_tts, auto_save_manager)?;

    Ok(())
}
