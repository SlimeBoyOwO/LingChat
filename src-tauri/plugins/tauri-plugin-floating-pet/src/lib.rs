//! # tauri-plugin-floating-pet
//!
//! 为 LingChat 的桌宠模式提供 **Android 系统级悬浮窗**能力。
//!
//! ## 背景
//!
//! 桌面端的桌宠是一个「透明 + 置顶 + 可点击穿透」的原生小窗口，
//! 由 `ling_chat_lib::api::pet` 实现。Android 没有等价的窗口概念，
//! 需要走 `SYSTEM_ALERT_WINDOW` + `WindowManager.addView()`，
//! 因此单独抽成一个插件，避免污染桌面端逻辑。
//!
//! ## 平台能力
//!
//! | 平台 | 支持 | 实现方式 |
//! |------|------|----------|
//! | Android | ✅ | `TYPE_APPLICATION_OVERLAY` 悬浮窗 + 内嵌 WebView |
//! | iOS | ❌ | 系统不允许跨 App 覆盖窗口 |
//! | 桌面端 | ❌ | 由 `api::pet` 的原生窗口实现 |
//!
//! 前端应先用 `is_supported` / `check_permission` 探测，
//! 再决定走悬浮窗路径还是桌面端窗口路径。

use std::sync::{Arc, Mutex};

use tauri::{
    AppHandle, Manager, Runtime,
    plugin::{Builder, TauriPlugin},
};

mod models;

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
use desktop as imp;
#[cfg(mobile)]
use mobile as imp;

pub use models::*;

/// 悬浮窗运行时状态，供 Rust 侧查询与幂等控制。
#[derive(Default, Clone)]
pub struct FloatingPetState {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    visible: bool,
    touchable: bool,
}

impl FloatingPetState {
    pub fn is_visible(&self) -> bool {
        self.inner.lock().map(|g| g.visible).unwrap_or(false)
    }

    pub fn is_touchable(&self) -> bool {
        self.inner.lock().map(|g| g.touchable).unwrap_or(false)
    }

    /// 仅供 `mobile.rs` 调用。桌面端构建时 `mobile.rs` 不参与编译，
    /// 因此这里显式放行 dead_code 警告（而非删掉或改结构）。
    #[cfg_attr(desktop, allow(dead_code))]
    fn set_visible(&self, visible: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.visible = visible;
        }
    }

    #[cfg_attr(desktop, allow(dead_code))]
    fn set_touchable(&self, touchable: bool) {
        if let Ok(mut g) = self.inner.lock() {
            g.touchable = touchable;
        }
    }
}

// ─── 命令 ────────────────────────────────────────────────────

#[tauri::command]
fn is_supported<R: Runtime>(app: AppHandle<R>) -> bool {
    imp::is_supported(&app)
}

#[tauri::command]
fn check_permission<R: Runtime>(app: AppHandle<R>) -> Result<bool> {
    imp::check_permission(&app)
}

/// 跳转到系统的「显示在其他应用上层」授权页。
///
/// 该权限是 Android 的特殊权限，**无法通过运行时弹窗申请**，
/// 只能引导用户到设置页手动开启。前端应在 `show` 失败于
/// `PermissionDenied` 时调用本命令，并在 App 恢复前台后重新
/// `check_permission` 确认结果。
#[tauri::command]
fn request_permission<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    imp::request_permission(&app)
}

#[tauri::command]
fn show<R: Runtime>(
    app: AppHandle<R>,
    args: ShowArgs,
    state: tauri::State<'_, FloatingPetState>,
) -> Result<()> {
    if !imp::is_supported(&app) {
        return Err(Error::NotSupported);
    }
    // 幂等：已显示时先隐藏，避免重复 addView 造成窗口泄漏
    if state.is_visible() {
        let _ = imp::hide(&app, &state);
    }
    imp::show(&app, args, &state)
}

#[tauri::command]
fn hide<R: Runtime>(app: AppHandle<R>, state: tauri::State<'_, FloatingPetState>) -> Result<()> {
    if !state.is_visible() {
        return Ok(());
    }
    imp::hide(&app, &state)
}

#[tauri::command]
fn move_pet<R: Runtime>(
    app: AppHandle<R>,
    args: MoveArgs,
    state: tauri::State<'_, FloatingPetState>,
) -> Result<()> {
    imp::move_pet(&app, args, &state)
}

#[tauri::command]
fn set_size<R: Runtime>(
    app: AppHandle<R>,
    args: SizeArgs,
    state: tauri::State<'_, FloatingPetState>,
) -> Result<()> {
    imp::set_size(&app, args, &state)
}

#[tauri::command]
fn set_touchable<R: Runtime>(
    app: AppHandle<R>,
    touchable: bool,
    state: tauri::State<'_, FloatingPetState>,
) -> Result<()> {
    imp::set_touchable(&app, touchable, &state)
}

#[tauri::command]
fn is_visible(state: tauri::State<'_, FloatingPetState>) -> bool {
    state.is_visible()
}

/// 展开 / 收起桌宠窗口。
///
/// 手机端没有鼠标悬停，展开动作由前端「点击头像」触发，
/// 对应桌面端 `mouseenter/mouseleave` 的自动展开。
#[tauri::command]
fn set_expanded<R: Runtime>(
    app: AppHandle<R>,
    expanded: bool,
    state: tauri::State<'_, FloatingPetState>,
) -> Result<()> {
    imp::set_expanded(&app, expanded, &state)
}

/// 查询平台能力、授权状态与**窗口几何**的聚合接口。
///
/// Android 上 `detached` / `scale` / `width` / `height` 只有 Kotlin 侧知道，
/// 因此优先转发给它；桌面端会返回 `NotSupported`，回落到纯 Rust 状态。
///
/// 前端**轮询**这个命令而不是等原生推事件：页面 → 原生的 Tauri IPC 是稳的
/// （点击、发消息都走它），而原生 → 页面的 `evaluateJavascript` 在搬运/收回
/// 前后并不可靠。
#[tauri::command]
fn status<R: Runtime>(app: AppHandle<R>, state: tauri::State<'_, FloatingPetState>) -> PetStatus {
    if let Ok(status) = imp::status(&app, &state) {
        return status;
    }

    let supported = imp::is_supported(&app);
    PetStatus {
        supported,
        granted: if supported {
            imp::check_permission(&app).unwrap_or(false)
        } else {
            false
        },
        visible: state.is_visible(),
        detached: false,
        scale: 1.0,
        width: 0.0,
        height: 0.0,
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("floating-pet")
        .invoke_handler(tauri::generate_handler![
            is_supported,
            check_permission,
            request_permission,
            show,
            hide,
            move_pet,
            set_size,
            set_touchable,
            set_expanded,
            is_visible,
            status,
        ])
        .setup(|app, api| {
            app.manage(FloatingPetState::default());

            // 把 Kotlin 侧实现注册给 Tauri，拿到 PluginHandle。
            // 移动端所有命令都通过这个 handle 转发到原生（见 mobile.rs）；
            // 没有这一步，run_mobile_plugin 无从调用。
            #[cfg(target_os = "android")]
            {
                let handle =
                    api.register_android_plugin(ANDROID_PLUGIN_IDENTIFIER, ANDROID_CLASS_NAME)?;
                app.manage(MobilePluginHandle::<R>(handle));
            }

            // 非 Android 平台（含 iOS）保留占位，避免 cfg 分支导致
            // FloatingPetState 之外的引用不一致。
            #[cfg(not(target_os = "android"))]
            let _ = api;

            Ok(())
        })
        .build()
}

/// Kotlin 插件所在包名，需与 `android/src/main/java/.../FloatingPetPlugin.kt` 的
/// `package` 声明一致。
#[cfg(target_os = "android")]
const ANDROID_PLUGIN_IDENTIFIER: &str = "com.noiq.floatingpet";

/// Kotlin 插件类名（不带包名）。
#[cfg(target_os = "android")]
const ANDROID_CLASS_NAME: &str = "FloatingPetPlugin";

/// 持有移动端原生插件的 `PluginHandle`，供命令转发时取用。
#[cfg(target_os = "android")]
pub struct MobilePluginHandle<R: Runtime>(pub tauri::plugin::PluginHandle<R>);

#[cfg(target_os = "android")]
impl<R: Runtime> MobilePluginHandle<R> {
    pub fn run<T: serde::de::DeserializeOwned>(
        &self,
        command: impl AsRef<str>,
        payload: impl serde::Serialize,
    ) -> Result<T> {
        self.0
            .run_mobile_plugin::<T>(command, payload)
            .map_err(|e| Error::Native(e.to_string()))
    }
}

/// 扩展 trait：`app.floating_pet()` 直接拿到状态。
pub trait FloatingPetExt<R: Runtime> {
    fn floating_pet(&self) -> tauri::State<'_, FloatingPetState>;
}

impl<R: Runtime, T: Manager<R>> FloatingPetExt<R> for T {
    fn floating_pet(&self) -> tauri::State<'_, FloatingPetState> {
        self.state::<FloatingPetState>()
    }
}
