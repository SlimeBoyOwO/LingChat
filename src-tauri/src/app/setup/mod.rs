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
use ling_chat_main::AppState;
use ling_chat_main::{ai_service, api, cast, data_dir, lan_sync, resource_sync, utils};
use tauri::Manager;

mod asr;
mod background;
mod build;
mod data;
mod voice_cleanup;

/// 与 `?` 等价，但失败时先把**完整错误链**写进 logcat。
///
/// Android 上 setup 闭包返回 `Err` 只会让 Tauri panic 出最外层消息
/// （`Failed to setup app: {e}`，anyhow 的 `Display` 不含 cause），而 tracing
/// 输出通向设备上不可见的 stdout。没有这一步，真机排障只能看到
/// "database migrations failed" 这类笼统结论，看不到真正的 SQL 错误。
///
/// 统一用 `Box<dyn std::error::Error>` 是因为 `Box<dyn Error>` 自身**不**实现
/// `std::error::Error`（`impl Error for Box<E>` 要求 `E: Sized`），泛型约束无法
/// 同时容纳它与 `anyhow::Error`；而 shell 的调用点本就把各类错误经 `?` 装箱成
/// `Box<dyn Error>`，因此这里直接以装箱类型为接口。
fn tried<T>(
    step: &str,
    result: Result<T, Box<dyn std::error::Error>>,
) -> Result<T, Box<dyn std::error::Error>> {
    if let Err(error) = &result {
        crate::app::platform::early_log(&format!("setup: {step} 失败: {error}"));
        let mut source = error.source();
        while let Some(cause) = source {
            crate::app::platform::early_log(&format!("setup:   caused by: {cause}"));
            source = cause.source();
        }
    }
    result
}

/// 执行 Tauri setup 阶段的全部初始化。
pub fn setup(
    app: &mut tauri::App<tauri::Wry>,
    log_filter: LogFilterHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    // 启动早期诊断：tracing 就绪前的阶段只能靠 logcat 留痕（见 platform::early_log）
    crate::app::platform::early_log("setup: 进入");

    // 设置日志桥接的应用句柄
    utils::log_bridge::set_app_handle(app.handle().clone());

    // 插件脚本在独立线程执行，需要一份全局句柄才能访问宿主状态
    ling_chat_plugins::set_app_handle(app.handle().clone());

    // 回复信号：宿主每产出一条 ai:reply 就经此回调转发给订阅的插件。generator
    // 线程拿不到 AppHandle，故用全局回调；回调内自行 spawn 到 async runtime。
    {
        let handle = app.handle().clone();
        ling_chat_main::plugin_contract::set_reply_hook(std::sync::Arc::new(move |resp| {
            let handle = handle.clone();
            let resp = resp.clone();
            tauri::async_runtime::spawn(async move {
                ling_chat_plugins::signal::emit_ai_reply(&handle, &resp).await;
            });
        }));
    }

    // 提前初始化数据目录缓存，以便在数据层引导之前
    // 将其传递给独立的本地 TTS crate。
    data_dir::init_data_dir(app.handle());
    crate::app::platform::early_log("setup: 数据目录已解析");

    // ONNX Runtime：定位 onnxruntime.dll 并显式加载
    // （仅 Windows 的 load-dynamic 模式，兼容无 AVX2 的旧 CPU，如三代酷睿；
    //  非 Windows 走 download-binaries 静态链接，无需此调用）。
    // 必须在任何 ort::Session 创建之前调用。
    #[cfg(target_os = "windows")]
    utils::onnx::init_onnx_runtime(app.handle());

    // 管理各种状态
    app.manage(api::pet::HitTestState::default());
    app.manage(api::pet::BubbleSideState::default());
    app.manage(resource_sync::ResourceSyncState::default());
    app.manage(lan_sync::LanSyncState::default());
    app.manage(cast::CastManager::default());
    app.manage(utils::cpu_perf::CpuDetectionCache::new());
    app.manage(utils::gpu_perf::GpuDetectionCache::new());
    app.manage(api::role_archive::RoleArchiveState::default());
    app.manage(api::tts_sherpa::init());

    #[cfg(desktop)]
    app.manage(ai_service::asr::global_hotkey::GlobalHotkeyState::default());

    // Android 修复：Tauri 在 setup 闭包执行前已创建 webview 窗口，前端 invoke
    // 命令会在 IPC runtime worker 上立即 dispatch；如果 AppState 还没 manage
    // 就会 panic "state() called before manage()"。所以 setup 一开始就 manage
    // 一个空壳 AppState，引导完成后用真实值 fill。
    app.manage(AppState::empty());
    let rt = tried(
        "tokio runtime",
        tokio::runtime::Runtime::new().map_err(Into::into),
    )?;
    crate::app::platform::early_log("setup: tokio runtime 已建立");
    // 本地 TTS（SBV2 进程内实现）：解析路径、注册 State/开关并收敛运行时。
    let local_tts = tried(
        "本地 TTS bootstrap",
        ai_service::tts::local::setup::bootstrap(app).map_err(Into::into),
    )?;
    crate::app::platform::early_log("setup: 本地 TTS bootstrap 完成");
    let (db, app_config) = tried(
        "数据层 bootstrap",
        rt.block_on(data::bootstrap(app)).map_err(Into::into),
    )?;
    crate::app::platform::early_log("setup: 数据层引导完成");

    // 语音快捷键全局注册（失去焦点可用）由**前端界面门控**驱动：仅 /chat 与
    // /pet 界面注册，离开界面注销释放 OS 键位（见 composables/asr 的 chatActive
    // watch → asr_ptt_global_set_active 命令）。启动停在主菜单（门控未激活），
    // 此处无需注册；按键事件转发见 app::builder 的 with_handler 回调。

    // 初始化文件日志（从设置读取开关和保留天数）+ 应用 genai 调试开关
    crate::app::logging::apply_log_settings(app, &log_filter);
    crate::app::platform::early_log("setup: 文件日志已初始化");
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
    } = tried(
        "服务图构建",
        build::build_service_graph(app, &rt, db, app_config, Some(local_tts.runtime.clone()))
            .map_err(Into::into),
    )?;
    crate::app::platform::early_log("setup: 服务图构建完成");

    // 填充 AppState。块作用域必须保留：`State<'_>` 借用 `app`，
    // 而下面还要继续用 `app.handle()`。
    {
        let state = app.state::<AppState>();
        state.fill(inner);
    }

    // 依赖完整 AppState 的后台任务与处理器。
    tried(
        "后台任务",
        background::run(app, &rt, &local_tts, auto_save_manager),
    )?;
    crate::app::platform::early_log("setup: 后台任务已启动，setup 返回 Ok");

    Ok(())
}
