# 推理设备选择与枚举（DirectML / WebGPU）

## 1. 概述

LingChat 本地 TTS 支持在 ONNX Runtime 上选择推理硬件（CPU / GPU / 指定显卡）。
Windows 通过 DirectML + DXGI 枚举显卡；本特性为 **Linux 的 WebGPU 后端**补齐了
等价的设备枚举能力——用户能在设置里看到具体显卡型号（如 `NVIDIA GeForce RTX 4080`），
并选中某一特定显卡跑推理，与 Windows 上的 DirectML 体验一致。

相关提交：`dfb76212 feat: Linux/WebGPU 推理设备枚举（Vulkan 物理设备，与 Dawn deviceId 对齐）`

## 2. 平台能力映射

| 平台      | 推理后端                  | 设备枚举方式                                   | 可选设备                        |
| --------- | ------------------------- | ---------------------------------------------- | ------------------------------- |
| Windows   | DirectML                  | DXGI（`CreateDXGIFactory1` → `EnumAdapters1`） | cpu / gpu / npu / device:\<id\> |
| **Linux** | **WebGPU（Dawn→Vulkan）** | **Vulkan（`vkEnumeratePhysicalDevices`）**     | **cpu / gpu / device:\<id\>**   |
| macOS     | CoreML / WebGPU           | 无枚举                                         | cpu（CoreML 自动启用）          |
| Android   | CPU                       | 无枚举                                         | cpu                             |

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

### 4.1 依赖（`crates/ling-chat-main/Cargo.toml`）

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

Windows 端用 DXGI 枚举 DirectML 可用的 GPU，位于 `crates/ling-chat-main/src/utils/device.rs` 的
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

`tts_local_list_devices`（`crates/ling-chat-main/src/ai_service/tts/local/mod.rs`）
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

| 内容        | 路径                                                                                           |
| ----------- | ---------------------------------------------------------------------------------------------- |
| 枚举 / 解析 | `crates/ling-chat-main/src/utils/device.rs`                                                    |
| 后端命令    | `crates/ling-chat-main/src/ai_service/tts/local/mod.rs`（`tts_local_list_devices` 等）         |
| EP 组装     | `sbv2_core`（git 依赖 shadow01a/sbv2-api）的 `src/model.rs`（`webgpu` 分支，`with_device_id`） |
| 前端选择器  | `src/components/settings/pages/SettingsTts.vue`                                                |
| 依赖        | `crates/ling-chat-main/Cargo.toml`（Windows target `windows` DXGI / Linux target `ash`）       |

## 9. 桌面端 sherpa-onnx 的推理后端

sherpa-onnx（ZipVoice / Matcha / Kokoro 这些 TTS）与上面 sbv2 那条线是两套东西：它的
执行提供方是**编译期编进库里的**（`csrc/session.cc` 里按 `SHERPA_ONNX_ENABLE_*` 走），
所以「用哪个后端」取决于链接进来的是哪份库，而不是运行期配置：

| 平台         | 后端         | 说明                                                                                                            |
| ------------ | ------------ | --------------------------------------------------------------------------------------------------------------- |
| Android      | `nnapi`      | 官方 `-android` 包按 API 21 编，NNAPI 需要 27+，实测会被降级回 CPU（见 docs/android/migration.md）              |
| iOS / macOS  | `coreml`     | Apple 上 CoreML 编译期默认开启                                                                                  |
| Windows      | `dml` 或 CPU | 只有用 `pnpm tauri:build:win` 自编的 DirectML 版才是 `dml`，见 9.1                                              |
| Linux x86_64 | CPU          | sherpa 在 Linux 上只有 CUDA 一条 GPU 路，代价是 +235 MiB 安装包 / +384 MiB 安装后体积，按体积取舍已移除，见 9.3 |

编译期常量 `SHERPA_ONNX_PROVIDER`（`dml` / `cpu`）由 `crates/ling-chat-main/build.rs`
发射：它读 `SHERPA_ONNX_LIB_DIR` 里的 `sherpa-build.json`（自编脚本写的标记），以
**实际链接的那套库**为准。官方预编译包没编 DirectML，那种情况下就算用户打开了「使用
GPU」也不会把 `dml` 交给 sherpa —— 否则只会打一行 fallback 日志再回落 CPU，等于把开关
变成假的。

### 9.1 Windows DirectML：构建时下源码 + 自编（`pnpm tauri:build:win`）

官方预编译的 Windows sherpa **没开 DirectML**（`CMakeLists.txt` 里
`option(SHERPA_ONNX_ENABLE_DIRECTML ... OFF)` 默认关，上游 `windows-x64.yaml` /
`build-wheels-win-x64.yaml` 也都没传这个开关），而应用**已经**为情绪模型随包了带 DML EP
的 `onnxruntime.dll`（WinML 2.4.89 内含 ORT 1.27.1）与 `DirectML.dll`。于是只要把 sherpa
自编一次、把 DML 那段代码编进去，就能复用这两个 DLL，净增量只有 sherpa 自己的 DLL
（约 5 MB），而不是 CUDA 包那样 +235 MB。

`scripts/build-desktop-sherpa-dml.mjs` 做三件事，**都在构建时**：

1. 按 `Cargo.lock` 里 `sherpa-onnx-sys` 的锁定版本下源码 tarball 到
   `target/sherpa-onnx-directml/`，解压后核对 `SHERPA_ONNX_VERSION` 一致；
2. 按 `patches/sherpa-onnx/*.patch` 顺序打补丁 —— 仓库里只存 diff，不留改过的源码树。
   DirectML 只要 cmake 开关、不需要动 C++，所以该目录现在是空的；将来要改 sherpa 的
   C++ 时把 `.patch` 丢进去即可（补丁内容变了会自动让缓存失效）；
3. cmake 自编：`-DSHERPA_ONNX_ENABLE_DIRECTML=ON -DBUILD_SHARED_LIBS=ON -A x64`
   （必须用 Visual Studio 生成器：sherpa 的 directml cmake 硬性要求
   `CMAKE_VS_PLATFORM_NAME=X64`）。ORT 走 cmake 的「预装」分支
   （`SHERPA_ONNXRUNTIME_INCLUDE_DIR` / `SHERPA_ONNXRUNTIME_LIB_DIR`）指向 WinML 包抽出
   的头文件与 x64 import lib：`scripts/ensure-onnxruntime.mjs` 顺手抽到
   `src-tauri/binaries/winml-ort/`，版本与运行时那份 DLL 天然一致。注意官方 ORT NuGet 的
   `build/native/include` 里**没有** `dml_provider_factory.h`，只有 DirectML 版才有。

为什么准备动作必须在 cargo 之前：cargo 的 build script 只能把信息传给**依赖它的**包，
没法把自己的环境变量注入给依赖 crate 的 build.rs，而上游 `sherpa-onnx-sys` 只认
`SHERPA_ONNX_LIB_DIR`。所以 `pnpm tauri:build:win` = `node
scripts/build-desktop-sherpa-dml.mjs --build`：先把库准备好、把 `SHERPA_ONNX_LIB_DIR`
设进子进程环境，再用 `--features sherpa-shared` 跑 tauri build。首次自编约十几分钟
（sherpa 全量编译），之后走缓存。

需要：Visual Studio（MSVC + Windows 10 SDK，`dml_provider_factory.h` 要 `d3d12.h`）、
cmake、网络。**本机（Linux）无法验证这条路径**：编不了也测不了，只能在 Windows 上跑。

### 9.2 打包

- Windows：sherpa 是动态库，DLL 必须跟安装包走。`scripts/build-desktop-sherpa-dml.mjs`
  调 `scripts/stage-desktop-runtime-libs.mjs`（同一份逻辑也挂在
  `build.beforeBundleCommand`）把 `sherpa-onnx-*.dll` 暂存到 `src-tauri/.runtime-libs/`，
  并生成 `--config` 覆盖（`src-tauri/tauri.sherpa-dml.conf.json`）逐文件映射到资源根目录
  （Windows 只在 exe 同目录找 DLL）。`onnxruntime.dll` / `DirectML.dll` 仍由
  `tauri.windows.conf.json` 从 `src-tauri/binaries/` 映射。
- Linux：`scripts/stage-desktop-runtime-libs.mjs` 按可执行文件的 `DT_NEEDED` 把库暂存到
  `src-tauri/.runtime-libs/`，再由 `tauri.conf.json` 的 `bundle.linux.*.files` 映射到
  `/usr/lib`。**不能放在可执行文件旁边**：`cargo:rustc-link-arg` 加的 `$ORIGIN` rpath
  到不了最终二进制（实测无 RPATH/RUNPATH），而加载器不搜可执行文件所在目录。目前 Linux
  只需要暂存 ort WebGPU 用到的 `libwebgpu_dawn.so`（sherpa 是静态 CPU 库）。
- macOS / iOS：核心库静态链接，无需暂存。

### 9.3 历史：已移除的 Linux CUDA 变体

2026-10 之前桌面端还支持链接**官方 CUDA 包**（`pnpm tauri:build:gpu`、
`scripts/fetch-sherpa-onnx-libs.mjs`、`SHERPA_ONNX_GPU=cuda12|cuda13`、编译期常量
`GPU_VARIANT`）。当时的实测数据（GTX 1060 3GB / Pascal CC 6.1 / `cuda12`）：

- provider 正常加载、权重进显存（约 400~900MB），推理期间 `nvidia-smi` 利用率峰值 78%
  —— ORT 1.27 的 CUDA 12 包内含 sm_61 内核，Pascal 可用；
- matcha 中文长句（72 字 / 12.7s 音频）：1.01s → 0.36s（约 2.8x）；
- kokoro int8：几乎无差别（21.1s → 19.4s），int8 算子基本落回 CPU，不建议为它开 GPU。

体积实测（`.deb` 字节数与 `control` 里的 `Installed-Size`）：CUDA 版 452,180,930 B /
680,545 KiB；CPU 版 205,665,150 B / 287,483 KiB —— 净增约 +235 MiB 安装包、+384 MiB
安装后体积，其中 `libonnxruntime_providers_cuda.so` 一个就占 92%。

移除原因：体积与「只有 NVIDIA 用户受益」的性价比。顺带记一条否决掉的捷径——本仓库的
情绪模型在 Linux 用的是 ort 的 WebGPU（Dawn→Vulkan），但 sherpa 认得的 provider 只有
`cpu/cuda/coreml/directml/nnapi/xnnpack/trt/qnn/rknpu/spacemit`，**没有 webgpu/vulkan**，
而且 EP 是编译期写死的，所以那份 ORT 借不过来。
