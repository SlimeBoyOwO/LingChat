//! Android 实现：把命令转发给原生 Kotlin 插件执行。
//!
//! iOS 与桌面端走 `NotSupported`——iOS 从系统层面就不允许第三方 App
//! 创建跨 App 的覆盖窗口，桌面端则由 `api::pet` 的原生窗口实现。

#[cfg(target_os = "android")]
use tauri::{AppHandle, Manager, Runtime};

use crate::FloatingPetState;
#[cfg(target_os = "android")]
use crate::MobilePluginHandle;
use crate::models::*;

#[cfg(target_os = "android")]
pub fn is_supported<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
}

#[cfg(not(target_os = "android"))]
pub fn is_supported<R: Runtime>(_app: &AppHandle<R>) -> bool {
    false
}

/// 取原生插件 handle 并转发调用。
///
/// handle 由 `init()` 的 setup 阶段注册并 manage，这里通过 `AppHandle`
/// 反查。命令名与 Kotlin 侧 `@Command` 注解的方法名一一对应。
#[cfg(target_os = "android")]
fn call<R: Runtime, P: serde::Serialize, T: serde::de::DeserializeOwned>(
    app: &AppHandle<R>,
    method: &str,
    payload: P,
) -> Result<T> {
    let handle = app.state::<MobilePluginHandle<R>>();
    handle.run::<T>(method, payload)
}

#[cfg(not(target_os = "android"))]
fn call<R: Runtime, P: serde::Serialize, T: serde::de::DeserializeOwned>(
    _app: &AppHandle<R>,
    _method: &str,
    _payload: P,
) -> Result<T> {
    Err(Error::NotSupported)
}

pub fn check_permission<R: Runtime>(app: &AppHandle<R>) -> Result<bool> {
    call(app, "checkPermission", ())
}

#[cfg(target_os = "android")]
pub fn request_permission<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    call(app, "requestPermission", ())
}

#[cfg(not(target_os = "android"))]
pub fn request_permission<R: Runtime>(_app: &AppHandle<R>) -> Result<()> {
    Err(Error::NotSupported)
}

pub fn show<R: Runtime>(
    app: &AppHandle<R>,
    args: ShowArgs,
    state: &FloatingPetState,
) -> Result<()> {
    call::<R, _, ()>(app, "show", args)?;
    state.set_visible(true);
    Ok(())
}

pub fn hide<R: Runtime>(app: &AppHandle<R>, state: &FloatingPetState) -> Result<()> {
    call::<R, _, ()>(app, "hide", ())?;
    state.set_visible(false);
    Ok(())
}

pub fn move_pet<R: Runtime>(
    app: &AppHandle<R>,
    args: MoveArgs,
    state: &FloatingPetState,
) -> Result<()> {
    if !state.is_visible() {
        return Err(Error::NotVisible);
    }
    call::<R, _, ()>(app, "movePet", args)
}

pub fn set_size<R: Runtime>(
    app: &AppHandle<R>,
    args: SizeArgs,
    state: &FloatingPetState,
) -> Result<()> {
    if !state.is_visible() {
        return Err(Error::NotVisible);
    }
    call::<R, _, ()>(app, "setSize", args)
}

/// 切换点击穿透。
///
/// `touchable = false` 时悬浮窗整体不接收触摸，手势落到下层 App；
/// `touchable = true` 时悬浮窗可交互。
///
/// 注意：Android 的该能力是**窗口级开关**，做不到桌面端那种按像素区域
/// 判定的精细穿透，因此悬浮窗应尽量贴合角色本身，避免大块透明留白。
pub fn set_touchable<R: Runtime>(
    app: &AppHandle<R>,
    touchable: bool,
    state: &FloatingPetState,
) -> Result<()> {
    if !state.is_visible() {
        return Err(Error::NotVisible);
    }
    state.set_touchable(touchable);
    call::<R, _, ()>(app, "setTouchable", TouchableArgs { touchable })
}

/// 让出 / 收回悬浮窗的输入焦点。
///
/// 悬浮窗展开时若一直摘着 `FLAG_NOT_FOCUSABLE`，它就会在**整个展开期间**持有
/// 输入焦点 —— 其他 App 的键盘弹不出来、返回键也先被它吃掉。
/// 现在由页面输入框的 `focus` / `blur` 驱动这个开关：
///
/// - `focused = true`  → Kotlin 侧摘掉 `FLAG_NOT_FOCUSABLE` 并弹出输入法；
/// - `focused = false` → 收起输入法并把标志加回去。
pub fn set_ime_focus<R: Runtime>(
    app: &AppHandle<R>,
    focused: bool,
    state: &FloatingPetState,
) -> Result<()> {
    if !state.is_visible() {
        return Err(Error::NotVisible);
    }
    call::<R, _, ()>(app, "setImeFocus", ImeFocusArgs { focused })
}

/// 展开 / 收起桌宠窗口。
///
/// 收起态只显示头像（约 1/6 屏宽），展开态容纳头像 + 输入框
/// （约 2/5 屏宽）。窗口尺寸由 Kotlin 侧按屏幕比例计算并 `updateViewLayout`。
///
/// 手机端没有鼠标悬停，因此由前端「点击头像」触发——这是与桌面端
/// `mouseenter/mouseleave` 自动展开的主要差异。
pub fn set_expanded<R: Runtime>(
    app: &AppHandle<R>,
    expanded: bool,
    state: &FloatingPetState,
) -> Result<()> {
    if !state.is_visible() {
        return Err(Error::NotVisible);
    }
    call::<R, _, ()>(app, "setExpanded", ExpandedArgs { expanded })
}

/// 改「桌宠大小」（设置页滑块）。
///
/// 悬浮窗宽度由原生独占，所以改系数必须回到原生重算 —— 见
/// [`crate::models::ScaleArgs`] 的说明。
pub fn set_scale<R: Runtime>(
    app: &AppHandle<R>,
    args: ScaleArgs,
    state: &FloatingPetState,
) -> Result<()> {
    if !state.is_visible() {
        return Err(Error::NotVisible);
    }
    call::<R, _, ()>(app, "setScale", args)
}

/// 查询实时状态与窗口几何。
///
/// 直接转发给 Kotlin 的 `status`：那里的 `detached` / `scale` 才是真值，
/// 而 Rust 侧 `FloatingPetState` 只是「前端调过 show/hide」的记录，
/// 原生自己把 WebView 搬回去时它并不知道。
///
/// 前端靠轮询这个命令自愈：不依赖任何原生 → 页面的事件推送，
/// 那条路在搬运/收回前后并不可靠。
pub fn status<R: Runtime>(app: &AppHandle<R>, _state: &FloatingPetState) -> Result<PetStatus> {
    call(app, "status", ())
}

/// 「屏幕视奸」（主动窥屏）取景：让原生截一帧，返回 JPEG 的**文件路径**。
///
/// ## 为什么是路径不是字节
///
/// 一帧约 100~300 KB。走 `run_mobile_plugin` 要 base64 塞进 JSON（体积再涨
/// 33%），两端还得各编解码一次 —— 而这条链路每 2 分钟就会走一次。原生写到
/// `cacheDir`、Rust 读文件，代价低得多。
///
/// ## 谁在用它
///
/// `crates/ling-chat-main::ai_service::screen_analyzer::capture_screen_as_jpeg()`
/// 的 Android 分支 —— 那个函数通过一个**全局注册的回调**拿到本函数
/// （见 `set_android_screen_capture` 的说明），所以 `ling-chat-main`
/// 不需要直接依赖本插件。
///
/// ## 失败语义
///
/// `ok = false` 是**正常结果**（用户还没开无障碍服务 / API < 30 / 截屏失败），
/// 不抛错；调用方据此降级。只有写文件失败才会 reject。
pub fn capture_screen<R: Runtime>(app: &AppHandle<R>) -> Result<ScreenCapture> {
    call(app, "captureScreen", ())
}
