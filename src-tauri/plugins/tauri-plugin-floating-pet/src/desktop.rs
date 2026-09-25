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
