//! 依赖完整 `AppState` 的后台任务与处理器。
//!
//! 原先内联在 `lib.rs` 的 setup 闭包中，**必须在 `AppState::fill` 之后**执行。
//! 整段搬迁，语句顺序（含 `spawn_preload` 先于窗口查找这类细节）原样保留。

use std::sync::Arc;

use crate::app::state::AppState;
use crate::{ai_service, api, cast, config};
use tauri::Manager;

/// 启动后台任务与处理器。
///
/// `auto_save_manager` 显式由参数传入而非从 `AppState` 反查：两者是同一个 `Arc`，
/// 但传参是纯搬运，也让本函数不依赖"`fill` 已执行"。
pub(super) fn run(
    app: &tauri::App<tauri::Wry>,
    rt: &tokio::runtime::Runtime,
    local_tts: &ai_service::tts::local::setup::LocalTtsBootstrap,
    auto_save_manager: Arc<tokio::sync::Mutex<ai_service::game_system::auto_save::AutoSaveManager>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // ASR 初始化：VAD 模型 + provider registry。失败仅 warn 不阻塞主程序。
    {
        let state = app.state::<AppState>();
        let asr_state = state.asr_state.clone();
        if let Err(e) = rt.block_on(super::asr::init_asr(app.handle(), &asr_state)) {
            tracing::warn!("[ASR] init_asr 失败，ASR 功能不可用: {e:#}");
        }
    }
    // 投屏自动启动：设置 cast.enabled=true 时，启动即打开投屏窗口并开启串流服务。
    // 延迟到主界面就绪后再做，避免投屏窗口先于主界面拿到场景快照。
    {
        let store = config::settings_store(app.handle()).ok();
        let cast_enabled = store
            .as_ref()
            .and_then(|s| s.get(config::keys::CAST_ENABLED))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if cast_enabled {
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                let cast = app_handle.state::<cast::CastManager>();
                if let Err(e) = cast::start_cast_server(&app_handle, &cast).await {
                    tracing::warn!("[Cast] 启动时自动开启投屏失败: {e}");
                }
            });
        }
    }

    // 插件携带资源收敛：把启用插件的人物/剧本/背景图同步进 DB / 剧本引擎 / 场景表。
    rt.block_on(api::plugins::refresh_plugin_content(app.handle()));

    // 延迟加载 DeBerta 直到应用主体挂载完成；
    // 如果在加载完成前有聊天请求到达，LocalTtsAdapter 的惰性引导仍然会运行，
    // 因此首次消息延迟是启动时加载的代价。
    ai_service::tts::local::setup::spawn_preload(&app.handle(), &local_tts);

    // 启动鼠标轮询点击穿透循环
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| tauri::Error::AssetNotFound("main window not found".to_string()))?;

    // 设置退出自动存档的关闭处理器
    ai_service::game_system::auto_save::AutoSaveManager::setup_close_handler(
        app.handle().clone(),
        window.clone(),
        auto_save_manager.clone(),
    );

    // 启动定期自动存档循环（间隔读自配置，热生效）
    tauri::async_runtime::spawn(async move {
        ai_service::game_system::auto_save::AutoSaveManager::run_periodic(auto_save_manager).await;
    });

    // 桌宠点击穿透轮询与 pet:cursor 鼠标广播——具体实现在 api::pet，
    // 入口文件不堆业务逻辑。
    #[cfg(desktop)]
    api::pet::spawn_hit_test_poll(window.clone());

    Ok(())
}
