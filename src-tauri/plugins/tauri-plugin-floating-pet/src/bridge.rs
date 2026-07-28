use serde::Serialize;
use tauri::Runtime;

/// 桥接到 Android Kotlin 插件的状态。
///
/// Android 上持有 Tauri 内部提供的 `PluginHandle<R>`，可通过
/// `run_mobile_plugin_async` 调用 Kotlin 侧的 `@Command` 处理函数。
/// 非 Android 平台持有 `PhantomData`，调用方需在编译期用 `#[cfg]` 隔离。
pub struct PetBridge<R: Runtime> {
    #[cfg(target_os = "android")]
    pub handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    pub _marker: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> PetBridge<R> {
    #[cfg(target_os = "android")]
    pub fn new(handle: tauri::plugin::PluginHandle<R>) -> Self {
        Self { handle }
    }

    #[cfg(not(target_os = "android"))]
    pub fn new() -> Self {
        Self { _marker: std::marker::PhantomData }
    }
}

/// 把 `serde_json::Value` 通过 JNI 送到 Kotlin 端。
///
/// `PluginHandle::run_mobile_plugin_async` 在非 Android 上不可用，本 trait
/// 借助 cfg 屏蔽；调用方按平台分流。
pub trait PetBridgeExt<R: Runtime> {
    fn invoke_android(
        &self,
        command: &'static str,
        payload: serde_json::Value,
    ) -> tauri::Result<()>;
}

impl<R: Runtime> PetBridgeExt<R> for PetBridge<R> {
    #[cfg(target_os = "android")]
    fn invoke_android(
        &self,
        command: &'static str,
        payload: serde_json::Value,
    ) -> tauri::Result<()> {
        self.handle.run_mobile_plugin(command, payload).map(|_| ())
    }

    #[cfg(not(target_os = "android"))]
    fn invoke_android(
        &self,
        _command: &'static str,
        _payload: serde_json::Value,
    ) -> tauri::Result<()> {
        // 在桌面 / iOS 上永远不会调用；保持接口一致。
        Ok(())
    }
}

/// 用于把 Rust 结构体映射成 `{"type": "tap", "payload": {...}}` 形式的事件负载。
#[derive(Debug, Serialize)]
pub struct EventEnvelope<'a> {
    pub r#type: &'a str,
    pub payload: serde_json::Value,
}
