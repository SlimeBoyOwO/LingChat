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
