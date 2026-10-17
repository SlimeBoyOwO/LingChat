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

## 9. 桌面端 sherpa-onnx 的 CUDA 变体

上面的 WebGPU/DirectML 是 sbv2 那条线；sherpa-onnx（TTS）走的是另一条线：官方预编译包
默认是 CPU 版，桌面还可以链接**官方 CUDA 包**，由 `SHERPA_ONNX_GPU` 选择变体。

### 9.1 怎么构建

```bash
pnpm tauri:build:gpu                 # = node scripts/package-desktop-sherpa-gpu-libs.mjs --variant=cuda12 --build
node scripts/package-desktop-sherpa-gpu-libs.mjs --variant cuda13   # 只准备库 + 打印命令
```

拆开看是三步（`scripts/package-desktop-sherpa-gpu-libs.mjs` 已串好）：

1. `scripts/fetch-sherpa-onnx-libs.mjs` 按**本次编译的版本**下官方 CUDA 包。版本从
   `Cargo.lock` 里 `sherpa-onnx-sys` 的锁定版本取，包名走 GitHub Release API 匹配
   （`...-cuda-<主版本>.…-{linux-x64-gpu,win-x64-cuda}.tar.bz2`，不硬编码
   `cudnn-9.x-onnxruntime1.27.1` 这类标签，升级 ORT/cuDNN 不用改代码），下载带
   断点续传 + 重试 + sha256 校验（digest 来自 Release API），直连失败会自动换杭州/官方
   镜像（`SHERPA_ONNX_DOWNLOAD_MIRROR` 可覆盖，`none` 关闭）。解压到
   `target/sherpa-onnx-prebuilt/<包名>/`，与上游 build.rs 的缓存同目录。
2. 把该目录导出成 `SHERPA_ONNX_LIB_DIR`：上游 `sherpa-onnx-sys` 的 build.rs 见到它就
   **跳过自己下载**，直接用这份库（`-L` + `dylib=` 链接）。
3. 用 `--features sherpa-shared` 走 shared 链接。官方 CUDA 包只有 `.so`/`.dll`，而上游
   build.rs 里 static 与 shared 同时开是硬报错，所以链接方式由 Cargo feature 决定，
   没开时是 static（Windows/Linux/macOS 的 CPU 构建都如此）。

不想用官方包（要自己控编译选项/打 C++ 补丁）可以自编：
`node scripts/fetch-sherpa-onnx-libs.mjs --variant=cuda12 --from=cmake`。它会下源码、
用 `-DSHERPA_ONNX_ENABLE_GPU=ON -DBUILD_SHARED_LIBS=ON` 编 sherpa 本体，并把产物汇总成
一个 lib 目录。注意 GPU 侧的执行提供方仍然是 ONNX Runtime 官方的预编译产物
（`onnxruntime-linux-x64-gpu_cuda12-<ver>-patched.zip`），所以自编只影响 sherpa 本体那层，
不改变运行期对 CUDA/cuDNN 的要求；也正因为这个包固定是 CUDA 12 版，自编只支持
`--variant=cuda12`（CUDA 13 请用官方预编译包）。

### 9.2 运行期要求与降级

目标机器需要 NVIDIA 驱动 + CUDA 12（或 13）+ cuDNN 9；库本身随安装包走（见 9.3）。
**缺 CUDA 运行时时，请求 `cuda` 会被 ONNX Runtime 抛 C++ 异常并 abort 整个进程**
（ORT 1.27 provider 加载失败穿不过 FFI），所以
`crates/ling-chat-main/src/ai_service/tts/adapters/sherpa_onnx.rs` 的 `cuda_ep_ready`
会先确认 provider 库与 CUDA 运行时都在，缺了就打警告并回退 CPU。探测方式**不能**改成
「自己 dlopen provider 库」：provider 的静态初始化会调用 `Provider_GetHost()`，该指针由
ORT 在加载 provider 之前设置，自己先加载会段错误。

编译期常量 `GPU_VARIANT`（`cpu` / `cuda12` / `cuda13`）由
`crates/ling-chat-main/build.rs` 发射，依据是 `SHERPA_ONNX_GPU` + `SHERPA_ONNX_LIB_DIR`
实际指向的目录（目录里没有 CUDA provider 库时会退回 `cpu` 并打 warning），运行时据此决定
要不要真的把 GPU provider 交给 sherpa。

### 9.3 打包

- Linux：`scripts/stage-desktop-runtime-libs.mjs` 按可执行文件的 `DT_NEEDED` 把库暂存到
  `src-tauri/.runtime-libs/`，再由 `tauri.conf.json` 的 `bundle.linux.*.files` 映射到
  `/usr/lib`。**不能放在可执行文件旁边**：`cargo:rustc-link-arg` 加的 `$ORIGIN` rpath 到不了
  最终二进制（实测无 RPATH/RUNPATH），而加载器不搜可执行文件所在目录。
- Windows：资源根目录即安装目录，`package-desktop-sherpa-gpu-libs.mjs` 生成 `--config`
  把 DLL 逐文件映射过去（与 `tauri.windows.conf.json` 里 `onnxruntime.dll` 的方式一致）。
- 打包入口与 `--features sherpa-shared`、`SHERPA_ONNX_LIB_DIR` 三者必须一致，
  `pnpm tauri:build:gpu` 内部已保证（`SHERPA_ONNX_GPU` 会传给 `beforeBundleCommand`，
  暂存脚本看到的变体与链接进来的包一致）。

### 9.4 实测（GTX 1060 3GB / Pascal CC 6.1，`cuda12`）

- provider 正常加载、权重进显存（约 400~900MB），推理期间 `nvidia-smi` 利用率峰值 78% ——
  ORT 1.27 的 CUDA 12 包内含 sm_61 内核，Pascal 可用。
- matcha 中文长句（72 字 / 12.7s 音频）：1.01s → 0.36s（约 2.8x）。
- kokoro int8：几乎无差别（21.1s → 19.4s），int8 算子基本落回 CPU，不建议为它开 GPU。
