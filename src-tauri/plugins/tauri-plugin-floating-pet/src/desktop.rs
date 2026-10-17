//! 桌面端实现。
//!
//! 桌面端的「桌宠」由 `api::pet` 模块用原生窗口（透明 + 置顶 + 点击穿透）
//! 实现，不需要系统级悬浮窗。这里的命令统一降级为 `NotSupported`，
//! 让前端可以用同一套 API 探测平台能力后走各自的实现路径。

use tauri::{AppHandle, Runtime};

use crate::FloatingPetState;
use crate::models::*;

pub fn is_supported<R: Runtime>(_app: &AppHandle<R>) -> bool {
    false
}

pub fn check_permission<R: Runtime>(_app: &AppHandle<R>) -> Result<bool> {
    Ok(false)
}

pub fn request_permission<R: Runtime>(_app: &AppHandle<R>) -> Result<()> {
    Err(Error::NotSupported)
}

pub fn show<R: Runtime>(
    _app: &AppHandle<R>,
    _args: ShowArgs,
    _state: &FloatingPetState,
) -> Result<()> {
    Err(Error::NotSupported)
}

pub fn hide<R: Runtime>(_app: &AppHandle<R>, _state: &FloatingPetState) -> Result<()> {
    Err(Error::NotSupported)
}

pub fn move_pet<R: Runtime>(
    _app: &AppHandle<R>,
    _args: MoveArgs,
    _state: &FloatingPetState,
) -> Result<()> {
    Err(Error::NotSupported)
}

pub fn set_size<R: Runtime>(
    _app: &AppHandle<R>,
    _args: SizeArgs,
    _state: &FloatingPetState,
) -> Result<()> {
    Err(Error::NotSupported)
}

pub fn set_touchable<R: Runtime>(
    _app: &AppHandle<R>,
    _touchable: bool,
    _state: &FloatingPetState,
) -> Result<()> {
    Err(Error::NotSupported)
}

/// 桌面端没有系统悬浮窗，也就不存在「悬浮窗抢走别处输入焦点」这个问题 ——
/// 桌面窗口本来就是前台普通窗口。这个命令只是给 Android 悬浮窗用的。
pub fn set_ime_focus<R: Runtime>(
    _app: &AppHandle<R>,
    _focused: bool,
    _state: &FloatingPetState,
) -> Result<()> {
    Err(Error::NotSupported)
}

/// 桌面端的展开/收起不由这个插件管：窗口尺寸归 `api::pet::set_pet_mode`，
/// 悬停展开输入框是纯前端行为（见 `PetMode.vue` 的 mouseenter/mouseleave）。
pub fn set_expanded<R: Runtime>(
    _app: &AppHandle<R>,
    _expanded: bool,
    _state: &FloatingPetState,
) -> Result<()> {
    Err(Error::NotSupported)
}

/// 桌面端同理：窗口尺寸由 `api::pet::set_pet_mode` 按 `pet.scale` 直接定，
/// 不需要经由插件改。这个命令只是给 Android 悬浮窗用的。
pub fn set_scale<R: Runtime>(
    _app: &AppHandle<R>,
    _args: ScaleArgs,
    _state: &FloatingPetState,
) -> Result<()> {
    Err(Error::NotSupported)
}

/// 桌面端没有系统悬浮窗，几何查询一律 `NotSupported`，
/// 由 `lib.rs` 回落到纯 Rust 状态（只有 `supported: false`）。
pub fn status<R: Runtime>(_app: &AppHandle<R>, _state: &FloatingPetState) -> Result<PetStatus> {
    Err(Error::NotSupported)
}

/// 「屏幕视奸」取景 —— 桌面端**不需要**这条通道。
///
/// 桌面端的截屏由 `crates/ling-chat-main` 的 `capture_screen_as_jpeg()`
/// 直接用 GDI（`BitBlt` + `GetDIBits`）完成，根本不经过本插件。
/// 这里保留一个同名函数只为让 `lib.rs` 的转发在两端都能编译。
pub fn capture_screen<R: Runtime>(_app: &AppHandle<R>) -> Result<ScreenCapture> {
    Err(Error::NotSupported)
}
