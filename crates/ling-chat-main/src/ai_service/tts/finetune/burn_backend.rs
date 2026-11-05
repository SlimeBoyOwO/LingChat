//! 微调训练用的 burn 后端接入点。
//!
//! 目标是把基座对齐训练的自动求导/优化器从 candle 迁到 burn，利用 Vulkan 做 GPU
//! 加速。张量类型统一用 burn 的 `Tensor<D>`（rank 在类型里，后端经运行时的
//! [`Device`](burn::tensor::Device) 调度）。
//!
//! burn 0.22 采用 flex 架构：`Tensor` 类型不带 backend 泛型，backend 通过
//! [`Device`](burn::tensor::Device) 注册与调度。训练固定用 Vulkan（wgpu 底层 /
//! SPIR-V），不启用 CPU 回退——这也是选用 Vulkan 而非 wgpu auto 的原因：明确钉在
//! Vulkan，设备起不来即明确失败，而不静默落到其它图形 API。

use burn::prelude::*;

/// 返回当前训练用的 burn 设备。
///
/// 固定走 Vulkan（`Device::vulkan(DeviceKind)`），由后端自动挑选可用设备。
/// 训练设备之间夹一层 autodiff（见 [`train_device_autodiff`]）。
pub fn train_device() -> Device {
    Device::vulkan(DeviceKind::DefaultDevice)
}

/// 带自动求导的训练设备。
///
/// 梯度下降需要在 vulkan 设备上启用反向传播上下文，训练张量统一用这里返回的
/// 设备创建。
pub fn train_device_autodiff() -> Device {
    train_device().autodiff()
}