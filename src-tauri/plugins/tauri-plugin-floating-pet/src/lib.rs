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

/// 让出 / 收回悬浮窗的输入焦点（由页面输入框的 focus / blur 驱动）。
///
/// 展开时**不能**无条件摘掉 `FLAG_NOT_FOCUSABLE`：那会让悬浮窗在整个展开期间
/// 持有输入焦点，把其他 App 的键盘和返回键都拦掉。改成只在真的要打字时才要焦点。
#[tauri::command]
fn set_ime_focus<R: Runtime>(
    app: AppHandle<R>,
    focused: bool,
    state: tauri::State<'_, FloatingPetState>,
) -> Result<()> {
    imp::set_ime_focus(&app, focused, &state)
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

/// 改「桌宠大小」（设置页滑块）。
///
/// 悬浮窗宽度由原生独占，前端改完 store 之后必须调这个命令让原生
/// 按新系数重算窗口宽度，否则窗口尺寸不变、滑块形同虚设。
#[tauri::command]
fn set_scale<R: Runtime>(
    app: AppHandle<R>,
    args: ScaleArgs,
    state: tauri::State<'_, FloatingPetState>,
) -> Result<()> {
    imp::set_scale(&app, args, &state)
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

/// 「屏幕视奸」（主动窥屏）取景。
///
/// Android 独有：桌面端的截屏由 `ling-chat-main` 直接走 GDI，不经过本插件
/// （桌面端调用本命令会得到 `NotSupported`）。
///
/// 返回的是 JPEG 的**文件路径**而不是字节 —— 理由见 `mobile.rs::capture_screen`。
/// 消费方是 `ling-chat-main::ai_service::screen_analyzer::capture_screen_as_jpeg()`，
/// 它通过 `set_android_screen_capture()` 注册的全局回调拿到本命令的结果，
/// 因此 `ling-chat-main` **不需要**反向依赖本插件。
///
/// ⚠️ 与其它命令不同，**未授权时也 resolve**（`ok = false`）——
/// 「用户还没同意 MediaProjection 的授权框」是正常状态，不是错误。
/// 这时先调一次 `request_screen_capture` 把框拉起来。
#[tauri::command]
fn capture_screen<R: Runtime>(app: AppHandle<R>) -> Result<ScreenCapture> {
    imp::capture_screen(&app)
}

/// 拉起系统的屏幕截取授权框：「允许 LingChat 开始截取您的屏幕内容吗？」
///
/// Android 独有（桌面端 GDI 截屏不需要授权，会得到 `NotSupported`）。
///
/// ## 这是「两步走」的第一步
///
/// `MediaProjection` 只能从一次用户授权的结果里换出来，而那次授权必须由某个
/// Activity 用 `startActivityForResult` 发起。所以弹框和截图没法合成一步：
///
/// ```text
/// request_screen_capture → 弹框
/// 用户点「立即开始」      → MainActivity.onActivityResult → 单例持有 token
/// capture_screen         → 此时才可能 ok = true
/// ```
///
/// ## 返回值的边界
///
/// 只说明**弹框有没有拉起来**（`prompted` / `already_granted` / `unsupported`），
/// 不说明用户点了什么 —— 弹框是异步的，选择结果只落到原生侧。
/// 前端要判断「到底能不能截」，得再调一次 `capture_screen` 看 `ok` / `reason`。
#[tauri::command]
fn request_screen_capture<R: Runtime>(app: AppHandle<R>) -> Result<ScreenCapturePermission> {
    imp::request_screen_capture(&app)
}

/// 屏幕截取授权是否已经拿到 —— **只读查询，不会截屏**。
///
/// 前端进桌宠前用它做门禁：没授权就先弹说明 + 拉系统授权框，
/// 而不是等用户进了桌宠、按了相机键才发现截不了。
///
/// 为什么不用 `capture_screen` 来探测：那条命令会**真的拍一帧**
/// （临时建 VirtualDisplay、等帧、压 JPEG，几百毫秒到几秒）。
/// 只为问一句「能不能截」不值得拍下来。
#[tauri::command]
fn is_screen_capture_ready<R: Runtime>(app: AppHandle<R>) -> Result<ScreenCaptureReady> {
    imp::is_screen_capture_ready(&app)
}

/// 截一帧屏幕，返回 JPEG 字节。
///
/// 这是给 `ling-chat-main` 的截屏注入点用的**高层封装** —— 它把
/// 「调原生命令 → 拿到路径 → 读文件」三步收在一起，调用方只需一行。
///
/// 失败统一返回 `None`（未授权 / 等帧超时 / 截屏失败 / 读文件失败），
/// 不向调用方区分原因 —— 对它来说都是「这一轮没图，跳过」。具体原因在
/// `log` 里留痕。
///
/// ⚠️ 本函数会**读一次磁盘**，且内部那次原生调用是阻塞的（上限 3s 等帧，
/// 见 `ScreenCaptureManager.captureJpeg`）。别在主线程上用它。
pub fn capture_screen_bytes<R: Runtime>(app: &AppHandle<R>) -> Option<Vec<u8>> {
    let capture = match imp::capture_screen(app) {
        Ok(capture) => capture,
        Err(e) => {
            log::debug!("[floating-pet] 屏幕取景不可用: {e}");
            return None;
        },
    };

    if !capture.ok || capture.path.is_empty() {
        // 「用户还没同意授权框」会走到这里，是正常状态，不打 warn 免得刷屏
        log::debug!("[floating-pet] 屏幕取景未成功: reason={}", capture.reason);
        return None;
    }

    match std::fs::read(&capture.path) {
        Ok(bytes) => Some(bytes),
        Err(e) => {
            log::warn!("[floating-pet] 读取截图文件失败 ({}): {e}", capture.path);
            None
        },
    }
}

/// 拉起屏幕截取授权框，返回原生给的 `reason`（拿不到时 `None`）。
///
/// 给 app 层（`src-tauri/src/app/setup`）用的**高层封装** —— 因为
/// `request_screen_capture` 是个 `#[tauri::command]`（私有），而 app 层需要
/// 在「取景失败且还没授权」时主动补一次授权。
///
/// ⚠️ 内部是**同步阻塞**的原生调用（`run_mobile_plugin` 等 Kotlin resolve），
/// 别在主线程上用它。好在它不弹模态、只是把系统对话框拉起来就立刻返回。
pub fn request_screen_capture_permission<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    match imp::request_screen_capture(app) {
        Ok(result) => Some(result.reason),
        Err(e) => {
            log::debug!("[floating-pet] 拉起屏幕截取授权不可用: {e}");
            None
        },
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
            set_ime_focus,
            set_expanded,
            set_scale,
            is_visible,
            status,
            capture_screen,
            request_screen_capture,
            is_screen_capture_ready,
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
