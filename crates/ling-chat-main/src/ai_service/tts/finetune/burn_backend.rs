//! 微调训练用的 burn 后端接入点。
//!
//! 训练固定走原生图形 API，不启用 CPU 回退：桌面/Android 用 Vulkan，
//! Apple 用 Metal，按平台在 Cargo 与 cfg 层分别启用。

use std::path::Path;
use std::sync::OnceLock;

use burn::prelude::*;

static CACHE_ROOT_CONFIGURED: OnceLock<()> = OnceLock::new();

/// 把 cubecl 缓存根目录指到应用私有目录；仅首次调用生效。
pub fn configure_cache_root(root: &Path) {
    CACHE_ROOT_CONFIGURED.get_or_init(|| {
        if let Err(error) = std::fs::create_dir_all(root) {
            tracing::warn!(
                "[burn] 无法创建 cubecl 缓存目录 {}，沿用默认缓存: {error}",
                root.display()
            );
            return;
        }
        cubecl_environment::environment::set_root(root);
        tracing::info!("[burn] cubecl 缓存根目录: {}", root.display());
    });
}

/// 返回当前训练用的 burn 设备（Apple 平台，走 Metal）。
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn train_device() -> Device {
    Device::metal(DeviceKind::DefaultDevice)
}

/// 带自动求导的训练设备（Apple 平台，走 Metal）。
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn train_device_autodiff() -> Device {
    train_device().autodiff()
}

/// 返回当前训练用的 burn 设备（桌面/Android，走 Vulkan）。
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub fn train_device() -> Device {
    Device::vulkan(DeviceKind::DefaultDevice)
}

/// 带自动求导的训练设备（桌面/Android，走 Vulkan）。
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub fn train_device_autodiff() -> Device {
    train_device().autodiff()
}
