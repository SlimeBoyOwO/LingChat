# 推理设备选择与枚举（DirectML / WebGPU）

## 1. 概述

LingChat 本地 TTS 支持在 ONNX Runtime 上选择推理硬件（CPU / GPU / 指定显卡）。
Windows 通过 DirectML + DXGI 枚举显卡；本特性为 **Linux 的 WebGPU 后端**补齐了
等价的设备枚举能力——用户能在设置里看到具体显卡型号（如 `NVIDIA GeForce RTX 4080`），
并选中某一特定显卡跑推理，与 Windows 上的 DirectML 体验一致。

相关提交：`dfb76212 feat: Linux/WebGPU 推理设备枚举（Vulkan 物理设备，与 Dawn deviceId 对齐）`

## 2. 平台能力映射

| 平台 | 推理后端 | 设备枚举方式 | 可选设备 |
|---|---|---|---|
| Windows | DirectML | DXGI（`CreateDXGIFactory1` → `EnumAdapters1`）| cpu / gpu / npu / device:\<id\> |
| **Linux** | **WebGPU（Dawn→Vulkan）** | **Vulkan（`vkEnumeratePhysicalDevices`）** | **cpu / gpu / device:\<id\>** |
| macOS | CoreML / WebGPU | 无枚举 | cpu（CoreML 自动启用）|
| Android | CPU | 无枚举 | cpu |

## 3. 原理：Vulkan 枚举与 deviceId 对齐

Linux 的 WebGPU EP 由 ONNX Runtime 内嵌的 Dawn 实现，走 **Vulkan** 后端。
Dawn 的 `Instance::EnumerateAdapters` 按 Vulkan 物理设备的枚举顺序生成 adapter，
因此：

```
vkEnumeratePhysicalDevices 的索引 N  ⇔  Dawn adapter 索引 N  ⇔  WebGPU EP 的 deviceId N
```

与 Windows 上"DXGI adapter 索引 = DirectML device_id"的对齐方式同理
（DXGI 对齐已在上游验证过；Vulkan 对齐基于同序枚举假设，建议真机双显卡复核）。

每个物理设备返回：

- `deviceName` → 型号（如 `NVIDIA GeForce RTX 4080`、`llvmpipe (LLVM 21.1.8, 256 bits)`）
- `vendorID` / `deviceID` → PCI 厂商 / 设备 ID
- 枚举索引 → 设备 ID（对应配置串 `device:<id>`）

## 4. 实现

### 4.1 依赖（`src-tauri/Cargo.toml`）

```toml
# Windows：DXGI 枚举（DirectML 设备）
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.62.2", features = ["Win32_UI_WindowsAndMessaging", "Win32_Foundation", "Win32_Graphics_Gdi", "Win32_Graphics_Dxgi"] }

# Linux：Vulkan 枚举（WebGPU 设备）
[target.'cfg(target_os = "linux")'.dependencies]
ash = { version = "0.38", default-features = false, features = ["loaded"] }
```

- `windows` crate 的 `Win32_Graphics_Dxgi` feature 提供 DXGI 枚举接口（`IDXGIFactory1` / `IDXGIAdapter1`）；
- `ash` 是标准 Rust Vulkan 绑定，`loaded` feature 使它在**运行时**通过 `libloading`
  动态加载 `libvulkan.so.1`，无构建期 Vulkan 开发包依赖；系统无 Vulkan 时优雅降级为空列表。

### 4.2 Windows/DirectML 枚举（DXGI）

Windows 端用 DXGI 枚举 DirectML 可用的 GPU，位于 `src-tauri/src/utils/device.rs` 的
`#[cfg(target_os = "windows")]` 分支：

```rust
#[cfg(target_os = "windows")]
{
    use std::collections::HashSet;
    use windows::Win32::Graphics::Dxgi::*;
    let mut devices = Vec::new();
    let mut seen: HashSet<(u32, u32)> = HashSet::new();
    unsafe {
        // 1. 创建 DXGI Factory
        if let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() {
            // 2. 按索引枚举 adapter（EnumAdapters1 返回 Err 即枚举完）
            for i in 0u32.. {
                if let Ok(adapter) = factory.EnumAdapters1(i) {
                    // 3. 取 DXGI_ADAPTER_DESC1（Description / VendorId / DeviceId）
                    if let Ok(desc) = adapter.GetDesc1() {
                        let name = String::from_utf16_lossy(&desc.Description)
                            .trim_end_matches('\0')
                            .to_string();
                        // 4. 跳过软件渲染器 + 去重同一物理 GPU
                        if desc.VendorId != 0x1414
                            && seen.insert((desc.VendorId, desc.DeviceId))
                        {
                            devices.push(DeviceInfo {
                                id: i as i32,
                                name,
                                vendor_id: desc.VendorId,
                                device_id: desc.DeviceId,
                            });
                        }
                    }
                } else {
                    break;
                }
            }
        }
    }
    devices
}
```

要点：

- **id = DXGI adapter 索引**，与 DirectML 的 `device_id` 对齐（已在上游验证）；
- **软件渲染器过滤**：跳过 `VendorId == 0x1414`（Microsoft Basic Render Driver），
  它不能被 DirectML 用于真实推理；
- **去重**：Intel 混合显卡系统会把同一核显枚举两次（渲染 / 计算两个入口），
  按 `(VendorId, DeviceId)` 去重后只保留 id 最小的；
- **型号**：`DXGI_ADAPTER_DESC1.Description`（UTF-16），如 `NVIDIA GeForce RTX 4080`。

### 4.3 枚举（Linux/WebGPU，Vulkan）

```rust
#[cfg(target_os = "linux")]
fn list_vulkan_devices() -> Vec<DeviceInfo> {
    use ash::vk;
    // 1. 加载 libvulkan（失败 → 空列表 + warn）
    let entry = unsafe { ash::Entry::load() }?;
    // 2. 创建 VkInstance（无扩展，仅用于枚举）
    let instance = unsafe { entry.create_instance(&create_info, None) }?;
    // 3. 枚举物理设备
    let devices = unsafe { instance.enumerate_physical_devices() }?;
    // 4. 逐个取 VkPhysicalDeviceProperties（deviceName / vendor_id / device_id）
    for (i, pd) in devices.iter().enumerate() {
        props = unsafe { instance.get_physical_device_properties(*pd) };
        out.push(DeviceInfo { id: i as i32, name, vendor_id, device_id });
    }
    // 5. 销毁实例
}
```

`list_devices()` 按平台分派：Windows → DXGI；Linux → `list_vulkan_devices`；
其他平台 → 空列表。

`DeviceInfo` 结构（三平台共用）：

```rust
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub id: i32,          // 设备 ID（Windows=DXGI 索引 / Linux=Vulkan 索引）
    pub name: String,     // 型号
    pub vendor_id: u32,
    pub device_id: u32,
}
```

### 4.4 设备解析（`device.rs` 的 `parse_device`）

EP 由 `Cargo.toml` 的 `[target.'cfg(...)'.dependencies]` 按平台+架构自动启用
（Windows→DirectML、x86_64 Linux→WebGPU、arm64 macOS→CoreML），不再有顶层
`tts-*` feature，代码侧直接用平台判断：

```rust
// Windows 与 x86_64 Linux 支持 gpu / device:<id>
#[cfg(any(target_os = "windows", all(target_os = "linux", target_arch = "x86_64")))]
_ if s.starts_with("device:") => { /* → InferenceDevice::Specific(id) */ }
```

- `gpu` 同样仅 Windows + x86_64 Linux；
- `npu` 仅 Windows（DirectML）；
- aarch64 Linux / macOS / Android 只接受 `cpu`（macOS 的 CoreML 在推理时自动启用）。

### 4.5 前端（`src/components/settings/pages/SettingsTts.vue`）

- 特定显卡下拉列表的显示条件由 `v-if="isWindows"` 放宽为 `v-if="isWindows || isLinux"`；
- 设置页加载时 `listDevices()` 的调用同样放宽（Windows 走 DXGI，Linux 走 Vulkan）；
- GPU 选项标签按平台区分：Windows 显示 `GPU（DirectML）`，Linux 显示 `GPU（WebGPU）`
  （i18n key：`settings.tts.device.gpu` / `settings.tts.device.gpuWebgpu`）。

### 4.6 后端命令

`tts_local_list_devices`（`src-tauri/src/ai_service/tts/local/mod.rs`）
委托 `crate::utils::device::list_devices()`，前端经 `TtsLocal.listDevices()` 调用。

## 5. 设备选择与热切换流程

```
用户选 device:<id>
  → saveInferenceDevice（SettingsTts.vue @change）
  → tts_local_set_device
      → parse_device → InferenceDevice::Specific(id)
      → 持久化配置
      → engine.set_device(device)      // 只存值
      → engine.unload_all()            // 销毁所有旧 session
      → engine.init()                  // 重建 TTSModelHolder（berb 用新设备）
  → 下次合成 load_voice → 语音模型也用 holder.device 重建
```

- **bert（DeBERTa）** 热切换后立即用新设备重建；
- **语音模型** 懒加载，切换后首次合成才用新设备重建（首次会稍慢）；
- 前提：本地 TTS 已启用且 DeBERTa 已安装；否则设备只保存、下次启用生效。

## 6. 验证方法

### 6.1 无 GPU 环境（VM，lavapipe）

VM 上应枚举到软件 Vulkan 设备：

```text
device[0]: llvmpipe (LLVM 21.1.8, 256 bits)  vendor=0x10005 device=0x0000
```

### 6.2 物理机（真显卡）

应用「设置 → 本地 TTS」里应列出真实显卡型号。若系统有多块 GPU（核显 + 独显），
应能分别列出并独立选择。

### 6.3 双显卡验证 deviceId 是否真的生效

WebGPU EP 的 `deviceId` 选项是否被 ORT 采纳需真机双显卡复核——切换后观察日志：

```text
[sbv2_core] load_model_with_device device=Specific(1) ...
```

若日志确认 `Specific(1)` 且推理实际落在对应显卡上，则对齐成立。

## 7. 已知限制与注意事项

1. **deviceId 采纳需真机复核**：枚举索引与 Dawn adapter 对齐基于同序假设；
   ORT WebGPU EP 是否真正使用 `deviceId` 选择 adapter，需双显卡真机验证。
   （Windows DirectML 的 DXGI 对齐已在上游验证。）
2. **不过滤软件设备**：与 Windows 跳过 Basic Render Driver 不同，Linux 保留
   lavapipe（软件 Vulkan）——GPU-less 环境下它是唯一可用的 WebGPU 设备。
3. **macOS 不适用 Vulkan 枚举**：WebGPU 在 macOS 走 Metal，本枚举仅实现
   Linux（`cfg(target_os = "linux")`），macOS 保持空列表（通常单 GPU，无影响）。
4. **运行时依赖 libvulkan**：加载失败时 `list_devices` 返回空列表并打 warn，
   不影响其他功能。

## 8. 相关代码位置

| 内容 | 路径 |
|---|---|
| 枚举 / 解析 | `src-tauri/src/utils/device.rs` |
| 后端命令 | `src-tauri/src/ai_service/tts/local/mod.rs`（`tts_local_list_devices` 等）|
| EP 组装 | `src-tauri/patches/sbv2_core/src/model.rs`（`webgpu` 分支，`with_device_id`）|
| 前端选择器 | `src/components/settings/pages/SettingsTts.vue` |
| 依赖 | `src-tauri/Cargo.toml`（Windows target `windows` DXGI / Linux target `ash`）|
