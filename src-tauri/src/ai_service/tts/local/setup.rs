//! 本地 TTS 的启动装配逻辑。
//!
//! 集中管理路径解析、State/开关注册、运行时收敛与 DeBERTa 后台预加载，
//! 避免在主程序 `lib.rs` 中堆积实现细节。

use std::sync::Arc;
use std::time::Duration;

use tauri::{App, AppHandle, Emitter, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use super::engine::LocalTtsEngine;
use super::{
    load_configured_enabled, LocalTtsPaths, LocalTtsRuntime, LocalTtsState, LocalTtsSwitch,
};

/// 本地 TTS 启动装配产物。`runtime` 注入主服务，`engine/paths/switch`
/// 供后台预加载复用。
pub struct LocalTtsBootstrap {
    pub runtime: LocalTtsRuntime,
    pub engine: Arc<LocalTtsEngine>,
    pub paths: LocalTtsPaths,
    pub switch: LocalTtsSwitch,
}

/// 解析路径、确保磁盘布局、注册 State/开关，并收敛出共享运行时。
/// 目录不可用时弹错误对话框，本次启动停用本地 TTS。
pub fn bootstrap(app: &App) -> Result<LocalTtsBootstrap, String> {
    let tts_paths = LocalTtsPaths::resolve(
        app.handle(),
        crate::init::static_copy::get_data_dir().clone(),
    )
    .map_err(|e| format!("LocalTtsPaths::resolve: {e}"))?;

    let paths_available = match tts_paths.ensure() {
        Ok(()) => true,
        Err(e) => {
            tracing::error!(
                target: "tts_local",
                "failed to create local TTS data directories: {e}"
            );
            app.dialog()
                .message(format!(
                    "无法创建本地 TTS 数据目录，本次启动将停用本地 TTS。\n\n请检查磁盘空间和数据目录权限。\n\n详细错误：{e}"
                ))
                .title("本地 TTS 初始化失败")
                .kind(MessageDialogKind::Error)
                .show(|_| {});
            false
        }
    };

    let state = LocalTtsState::new(tts_paths);
    // `enable_local_tts` 直接存储在 `features.enable_local_tts` 下
    //（前端通过 `getEnvConfigByKey` 读取）；AppConfig 不拥有此字段。
    // 在此读取一次，以便进程内引擎以用户选择的状态启动。
    let switch = LocalTtsSwitch::new(paths_available && load_configured_enabled(app.handle()));
    app.manage(switch.clone());

    // 先读取持久化的推理设备配置，拿到设置后再初始化对应设备。
    // 这样引擎以用户上次的选择启动（而非默认 CPU），spawn_preload 初始化时
    // 直接使用这里确定的 device，无需运行时补救。
    if let Some(device) = super::read_configured_device(app.handle()) {
        tracing::info!(target: "tts_local", "bootstrap: using persisted device {:?}", device);
        // set_device 是 async 的，但这里在 setup 阶段引擎未并发使用，block_on 一次安全
        tauri::async_runtime::block_on(state.engine.set_device(device));
    } else {
        tracing::info!(target: "tts_local", "bootstrap: no persisted device, using CPU default");
    }

    let engine = state.engine.clone();
    let paths = state.paths.clone();
    app.manage(state);

    let runtime = LocalTtsRuntime::new(engine.clone(), paths.clone(), switch.clone());
    Ok(LocalTtsBootstrap {
        runtime,
        engine,
        paths,
        switch,
    })
}

/// 延迟加载 DeBERTa 直到应用主体挂载完成；如果在加载完成前有聊天请求
/// 到达，`LocalTtsAdapter` 的惰性引导仍然会运行，因此首次消息延迟是
/// 启动时加载的代价。
pub fn spawn_preload(app: &AppHandle, local: &LocalTtsBootstrap) {
    let preload = app.clone();
    let engine = local.engine.clone();
    let paths = local.paths.clone();
    let switch = local.switch.clone();
    tauri::async_runtime::spawn(async move {
        tokio::task::yield_now().await;
        if !switch.is_enabled() {
            tracing::info!(target: "tts_local", "local tts disabled, skipping preload");
            return;
        }
        if !paths.asset_present("deberta") {
            tracing::info!(target: "tts_local", "local tts assets missing, skipping preload");
            return;
        }

        // bootstrap 已确定推理设备（持久化配置），这里直接用，无需重读。
        // init 会用 engine 里已设置的 device 加载。
        tracing::info!(
            target: "tts_local",
            "preload: device={:?}",
            engine.device().await
        );

        if engine.is_ready().await {
            return;
        }
        match tokio::time::timeout(Duration::from_secs(15), engine.init(&paths)).await {
            Ok(Ok(())) => {
                tracing::info!(target: "tts_local", "deberta preloaded in background");
                let _ = preload.emit("tts://engine-ready", ());
            }
            Ok(Err(e)) => {
                tracing::warn!(
                    target: "tts_local",
                    "deberta preload failed (first synthesize will retry): {e}"
                );
            }
            Err(_) => {
                tracing::warn!(
                    target: "tts_local",
                    "deberta preload timed out after 15 seconds (first synthesize will retry)"
                );
            }
        }
    });
}
