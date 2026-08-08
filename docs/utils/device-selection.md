# 推理设备选择工具（utils/device）

模块：`src-tauri/src/utils/device.rs`

提供 ONNX 推理功能的**统一设备选择**能力。当前使用者：本地 TTS（`ai_service/tts/local`）。
其他使用 ONNX Runtime 推理的功能（如情绪识别 `ai_service/emotion`）应通过本模块
选择设备，避免各自复制 EP 配置、DXGI 枚举与配置读写逻辑。

## 能力概览

| 函数 | 作用 | 平台 |
|------|------|------|
| `parse_device(str)` | 解析设备字符串 → `InferenceDevice` | 全平台 |
| `device_to_string(InferenceDevice)` | 序列化设备为配置字符串 | 全平台 |
| `list_devices()` | 枚举 DirectML GPU 列表 | Windows（其他平台空） |
| `read_configured_device(app, key)` | 从 settings.json 读持久化设备 | 全平台 |
| `read_settings_string(app, key)` | 读任意 settings.json 字符串配置 | 全平台 |

## 类型

### InferenceDevice（来自 sbv2_core::model）

```rust
pub enum InferenceDevice {
    Cpu,          // CPU 推理（默认，全平台）
    Gpu,          // DirectML GPU（Windows）
    Npu,          // DirectML NPU（Windows）
    Specific(i32) // DirectML 指定设备（device_id）
}
```

设备字符串格式：`"cpu" | "gpu" | "npu" | "device:<id>"`。
`device:<id>` 的 id 来自 [`list_devices()`]（DXGI 枚举顺序，与 DirectML device_id 对齐）。

## 使用方式

### 1. 让 ONNX Session 用指定设备推理

```rust
use sbv2_core::model::InferenceDevice;
use ort::session::Session;

// 按用户选择构建 EP 列表（Gpu/Npu 用 DirectML，Cpu 纯 CPU）
fn build_ep(device: InferenceDevice) -> Vec<ort::ep::ExecutionProviderDispatch> {
    let mut exp = Vec::new();
    match device {
        InferenceDevice::Gpu => {
            exp.push(
                ort::ep::DirectML::default()
                    .with_device_filter(ort::ep::directml::DeviceFilter::Gpu)
                    .build(),
            )
        }
        InferenceDevice::Npu => {
            exp.push(
                ort::ep::DirectML::default()
                    .with_device_filter(ort::ep::directml::DeviceFilter::Npu)
                    .build(),
            )
        }
        InferenceDevice::Specific(id) => {
            exp.push(ort::ep::DirectML::default().with_device_id(id).build())
        }
        InferenceDevice::Cpu => {}
    }
    exp.push(ort::ep::CPU::default().build());
    exp
}

// Session 构建时传入 EP 列表
let session = Session::builder()?
    .with_execution_providers(build_ep(device))?   // 热切换 = 重建 Session
    .commit_from_file(model_path)?;
```

> 注意：EP 在 Session 创建时固化。切换设备需**重建 Session**（unload 后重新加载），
> 参考本地 TTS 的 `tts_local_set_device` 流程。

### 2. 枚举系统 GPU（用户选显卡）

```rust
use crate::utils::device::{list_devices, DeviceInfo};

let gpus: Vec<DeviceInfo> = list_devices();
for gpu in &gpus {
    println!("{}: {} (device_id {})", gpu.id, gpu.name, gpu.device_id);
}
// 用户选中的 id → device_string = format!("device:{}", gpu.id)
```

### 3. 读写持久化设备配置

```rust
use crate::utils::device::{read_configured_device, parse_device, device_to_string};

// 读（返回 Option<InferenceDevice>，未配置为 None）
let device = read_configured_device(&app, "features.local_tts_device")
    .unwrap_or(InferenceDevice::Cpu);

// 写（通过 settings store，TTS 的做法）
store.set("features.local_tts_device", device_to_string(device));
store.save()?;
```

### 4. 读任意 settings.json 字符串配置

```rust
use crate::utils::device::read_settings_string;

let some_setting = read_settings_string(&app, "llm.provider");
```

## 平台行为

- **Windows**：GPU/NPU 可用（DirectML），`list_devices()` 返回真实 GPU 列表
- **Android/Linux**：只有 CPU（`parse_device` 非 "cpu" 报错，`list_devices()` 空）
- 前端按平台显示设备选项（TTS 设置页用 `isWindows` 控制显示）

## 测试

`cargo test --lib utils::device` 覆盖：
- 设备字符串解析（cpu/gpu/npu/device:id/非法值）
- `device_to_string` 往返

## 配置键

TTS 当前用 `features.local_tts_device`。其他功能可复用本模块但**使用各自的 key**
（避免冲突），如 `features.emotion_device`。
