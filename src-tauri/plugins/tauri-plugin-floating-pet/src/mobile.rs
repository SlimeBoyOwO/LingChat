//! Android / iOS 实现：把命令转发给原生插件执行。
//!
//! 目前只实现了 Android（Kotlin）。iOS 侧走 `NotSupported`，
//! 因为 iOS 从系统层面就不允许第三方 App 创建跨 App 的覆盖窗口。

use tauri::{AppHandle, Runtime};

use crate::FloatingPetState;
use crate::models::*;

#[cfg(target_os = "android")]
pub fn is_supported<R: Runtime>(_app: &AppHandle<R>) -> bool {
    true
}

#[cfg(not(target_os = "android"))]
pub fn is_supported<R: Runtime>(_app: &AppHandle<R>) -> bool {
    false
}

/// 转发调用到原生插件。
///
/// Tauri 在移动端约定：Rust 侧用 `run_mobile_plugin`，Kotlin 侧用
/// `@Command` 注解的方法接收，方法名与这里的字符串一一对应。
#[cfg(target_os = "android")]
fn call<R: Runtime, P: serde::Serialize>(
    app: &AppHandle<R>,
    method: &str,
    payload: P,
) -> Result<()> {
    app.run_mobile_plugin::<()>(method, payload)
        .map_err(|e| Error::Native(e.to_string()))
}

#[cfg(not(target_os = "android"))]
fn call<R: Runtime, P: serde::Serialize>(
    _app: &AppHandle<R>,
    _method: &str,
    _payload: P,
) -> Result<()> {
    Err(Error::NotSupported)
}

#[cfg(target_os = "android")]
pub fn check_permission<R: Runtime>(app: &AppHandle<R>) -> Result<bool> {
    app.run_mobile_plugin::<bool>("checkPermission", ())
        .map_err(|e| Error::Native(e.to_string()))
}

#[cfg(not(target_os = "android"))]
pub fn check_permission<R: Runtime>(_app: &AppHandle<R>) -> Result<bool> {
    Ok(false)
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
    call(app, "show", args)?;
    state.set_visible(true);
    Ok(())
}

pub fn hide<R: Runtime>(app: &AppHandle<R>, state: &FloatingPetState) -> Result<()> {
    call(app, "hide", ())?;
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
    call(app, "movePet", args)
}

pub fn set_size<R: Runtime>(
    app: &AppHandle<R>,
    args: SizeArgs,
    state: &FloatingPetState,
) -> Result<()> {
    if !state.is_visible() {
        return Err(Error::NotVisible);
    }
    call(app, "setSize", args)
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
    call(app, "setTouchable", TouchableArgs { touchable })
}
