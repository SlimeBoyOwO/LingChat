//! 微调训练用的 burn 后端接入点。
//!
//! 目标是把基座对齐训练的自动求导/优化器从 candle 迁到 burn，利用 GPU
//! 加速。张量类型统一用 burn 的 `Tensor<D>`（rank 在类型里，后端经运行时的
//! [`Device`](burn::tensor::Device) 调度）。
//!
//! burn 0.22 采用 flex 架构：`Tensor` 类型不带 backend 泛型，backend 通过
//! [`Device`](burn::tensor::Device) 注册与调度。训练固定走原生图形 API，不启用
//! CPU 回退——设备起不来即明确失败，而不静默落到其它实施：
//!  - 桌面（Linux/Windows）与 Android：Vulkan（wgpu / SPIR-V）；
//!  - Apple（macOS/iOS）：Metal（wgpu / MSL，Apple 无原生 Vulkan）。
//!
//! 后端在 Cargo 层按平台分别启用（Apple 只开 `metal`、其它平台只开 `vulkan`），
//! 编译期用 `#[cfg(target_os)]` 选中对应设备实现。既避免运行时在苹果平台上请求
//! 不存在的 Vulkan 设备，也确保 macOS 不会被无谓拉进 wgpu 的 `spirv`（那会要求
//! 装 Vulkan SDK 才能编）。

use std::path::Path;
use std::sync::OnceLock;

use burn::prelude::*;

/// cubecl 持久化缓存根目录是否已经配置。
static CACHE_ROOT_CONFIGURED: OnceLock<()> = OnceLock::new();

/// 把 cubecl 的持久化缓存根目录指到应用私有目录。
///
/// Android 上进程通常没有 `HOME`/`XDG_CACHE_HOME`，cubecl 找不到用户缓存目录时
/// 会回退到 `std::env::temp_dir()`（Android 通常是 `/data/local/tmp`）。普通应用
/// 对该目录没有写权限，kernel 编译缓存只能退化成内存缓存，冷启动会反复重编。
///
/// 该函数幂等：只有第一次调用生效。目录创建失败时仅告警并保留 cubecl 默认
/// 行为，不影响训练能否运行。
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
///
/// Apple 走 `Device::metal(DeviceKind)`；桌面与 Android 走 Vulkan（见下面那个
/// 非 Apple 版本）。二者都只挑选可用的 GPU，由 wgpu 自动匹配设备。训练设备之间
/// 夹一层 autodiff（见 [`train_device_autodiff`]）。
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
///
/// 桌面与 Android 走 `Device::vulkan(DeviceKind)`，挑选可用的 GPU，由 wgpu
/// 自动匹配设备。训练设备之间夹一层 autodiff（见 [`train_device_autodiff`]）。
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub fn train_device() -> Device {
    Device::vulkan(DeviceKind::DefaultDevice)
}

/// 带自动求导的训练设备（桌面/Android，走 Vulkan）。
#[cfg(not(any(target_os = "macos", target_os = "ios")))]
pub fn train_device_autodiff() -> Device {
    train_device().autodiff()
}
