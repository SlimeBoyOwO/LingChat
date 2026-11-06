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
//! 同一份代码在编译期按 `cfg!(target_os)` 选中合适设备，避免运行时才在
//! 苹果平台上请求不存在的 Vulkan 设备而 panic。

use burn::prelude::*;

/// 返回当前训练用的 burn 设备。
///
/// 桌面与 Android 固定走 Vulkan（`Device::vulkan(DeviceKind)`）；Apple 平台
/// 走 Metal（`Device::metal(DeviceKind)`）。二者都只挑选可用的 GPU，由 wgpu
/// 自动匹配设备。训练设备之间夹一层 autodiff（见 [`train_device_autodiff`]）。
pub fn train_device() -> Device {
    if cfg!(any(target_os = "macos", target_os = "ios")) {
        Device::metal(DeviceKind::DefaultDevice)
    } else {
        Device::vulkan(DeviceKind::DefaultDevice)
    }
}

/// 带自动求导的训练设备。
///
/// 梯度下降需要在训练设备上启用反向传播上下文，训练张量统一用这里返回的
/// 设备创建。
pub fn train_device_autodiff() -> Device {
    train_device().autodiff()
}