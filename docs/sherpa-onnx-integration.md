# Sherpa-ONNX 集成指南

本文档介绍了如何在 LingChat 中集成 Sherpa-ONNX 作为内置语音引擎。

## 功能特性

- 🎤 **多种模型支持**：支持 VITS、FastSpeech2、Matcha-TTS、Kokoro、Kitten、ZipVoice（零样本克隆）、Pocket、Supertonic 等模型
- 🌐 **多语言支持**：支持中文、英文、日文等多种语言
- 🔧 **本地运行**：完全本地化，无需联网即可使用
- 🚀 **高性能**：基于 ONNX Runtime，支持 CPU 和 GPU 加速
- 📦 **模型管理**：内置模型下载、验证和管理功能

## 系统要求

- **操作系统**：Windows 10/11, macOS, Linux
- **内存**：至少 4GB RAM（推荐 8GB+）
- **存储**：每个模型约 500MB - 2GB
- **依赖**：
  - ONNX Runtime（已包含在项目中）
  - 7-Zip（用于解压模型文件）

## 快速开始

### 1. 下载模型

使用提供的脚本下载预训练模型：

```bash
# 查看可用模型列表
node scripts/download_sherpa_onnx_models.mjs --models

# 下载中文 VITS 模型
node scripts/download_sherpa_onnx_models.mjs vits-zh

# 下载英文 FastSpeech2 模型
node scripts/download_sherpa_onnx_models.mjs fastspeech2-en

# 查看已下载的模型
node scripts/download_sherpa_onnx_models.mjs --list
```

### 2. 配置模型

在 LingChat 的角色设置中，添加 Sherpa-ONNX 配置：

```json
{
  "tts_type": "sherpa-onnx",
  "sherpa_onnx_model_name": "vits-zh",
  "sherpa_onnx_model_type": "vits",
  "sherpa_onnx_lang": "zh",
  "sherpa_onnx_voice": "female",
  "sherpa_onnx_use_gpu": false
}
```

> 说明：模型目录为 `data/sherpa_onnx_models/<name>/`，由 `sherpa_onnx_model_name`
> 唯一确定；不存在 `sherpa_onnx_model_path` 配置项。

### 3. 使用 Sherpa-ONNX

在角色设置中选择 "sherpa-onnx" 作为 TTS 类型即可开始使用。

## 支持的模型

### VITS 模型

- **vits-zh**: 中文 VITS 模型（基于 AIShell3 数据集）
- **vits-en**: 英文 VITS 模型（基于 LJSpeech 数据集）

### FastSpeech2 模型

- **fastspeech2-zh**: 中文 FastSpeech2 模型
- **fastspeech2-en**: 英文 FastSpeech2 模型

### Matcha-TTS 模型

- **matcha-zh**: 中文 Matcha-TTS 模型
- **matcha-en**: 英文 Matcha-TTS 模型

### Kokoro / Kitten 模型

- **kokoro**: Kokoro 多语言模型（含 `voices.bin`）
- **kitten**: Kitten 模型（与 Kokoro 同结构的 `voices.bin`）

### ZipVoice 模型（零样本克隆）

- **zipvoice**: ZipVoice 中英模型（Emilia 数据集），支持参考音频零样本语音克隆：
  - `sherpa_onnx_ref_audio_path` + `sherpa_onnx_ref_text`

### Pocket / Supertonic 模型

- **pocket**: Pocket-TTS（`lm_flow`/`lm_main`/encoder/decoder/text_conditioner/vocab/token_scores）
- **supertonic**: Supertonic（duration_predictor/text_encoder/vector_estimator/vocoder/tts.json）

> 说明：引擎底层由 sherpa-onnx 提供这些模型族支持，但应用的预置下载目录仅收录
> VITS/FastSpeech2/Matcha/Kokoro/ZipVoice；Kitten/Pocket/Supertonic 需手动放入
> `data/sherpa_onnx_models/` 后使用。Tortoise 不受 sherpa-onnx 支持，请勿使用。

## 配置选项

### 基础配置

| 参数                         | 类型    | 默认值   | 描述                                                                         |
| ---------------------------- | ------- | -------- | ---------------------------------------------------------------------------- |
| `sherpa_onnx_model_name`     | String  | -        | 模型名称（对应 `data/sherpa_onnx_models/<name>/` 目录）                      |
| `sherpa_onnx_model_type`     | String  | "vits"   | 模型类型（vits/fastspeech2/matcha/kokoro/kitten/zipvoice/pocket/supertonic） |
| `sherpa_onnx_lang`           | String  | "zh"     | 语言代码                                                                     |
| `sherpa_onnx_voice`          | String  | "female" | 音色名称                                                                     |
| `sherpa_onnx_use_gpu`        | Boolean | false    | 是否使用 GPU/硬件加速                                                        |
| `sherpa_onnx_speed`          | Number  | 1.0      | 语速（0.5–2.0）                                                              |
| `sherpa_onnx_ref_audio_path` | String  | -        | 零样本参考音频路径（ZipVoice 等）                                            |
| `sherpa_onnx_ref_text`       | String  | -        | 参考音频对应的文本                                                           |

### 推理参数说明

- VITS/FastSpeech2 的 `noise_scale`(0.667)、`noise_scale_w`(0.8)、`length_scale`(1.0)
  等为引擎硬编码的官方推荐默认值，**不可通过配置调整**（如需调整可修改源码适配器）。
- 直接可调项为 `sherpa_onnx_speed`；`sid` 由 `sherpa_onnx_voice` 映射。
- 若模型目录内附带 Thrax 规则文件（`rule_fst.fst`/`rule.fst`/`rule.far`），
  适配器会自动挂载，提升长文本中日期、数字等的规范化效果。

## 高级用法

### 自定义模型

1. 下载自定义模型文件到 `data/sherpa_onnx_models/your-model-name/`
2. 确保对应模型类型的核心文件存在（如 VITS 需 `model.onnx`、ZipVoice 需 `fm_decoder.onnx`）
3. 创建 `config.json` 配置文件：

```json
{
  "model_type": "vits",
  "language": "zh",
  "voice": "custom_voice",
  "name": "Custom Model",
  "downloaded_at": "2024-01-01T00:00:00.000Z"
}
```

### GPU 加速

开启 `sherpa_onnx_use_gpu: true` 后，运行时按平台自动选择后端：
GPU 初始化失败会自动回退 CPU：

- Windows: `dml`（DirectML）
- macOS: `coreml`
- Linux x86_64: `cuda`
- Android: `nnapi`

> 不存在 `sherpa_onnx_device` 配置项，设备由系统自动选择。

### 批量处理

```javascript
// 批量下载多个模型
const models = ["vits-zh", "fastspeech2-en", "matcha-zh"];
for (const model of models) {
  await exec(`node scripts/download_sherpa_onnx_models.mjs ${model}`);
}
```

## 故障排除

### 常见问题

1. **模型下载失败**
   - 检查网络连接
   - 确认磁盘空间充足
   - 尝试使用代理或镜像源

2. **模型加载失败**
   - 检查模型文件完整性
   - 确认模型格式正确
   - 查看 ONNX Runtime 日志

3. **语音质量不佳**
   - 尝试不同的模型
   - 调整模型参数
   - 检查输入文本预处理

### 日志和调试

启用详细日志：

```bash
export RUST_LOG=debug
# 运行 LingChat
```

### 性能优化

- 使用 GPU 加速（如果可用）
- 调整模型参数以平衡质量和速度
- 预加载常用模型

## API 参考

### SherpaOnnxAdapter

主要的 TTS 适配器类，提供语音合成功能。

#### 方法

- `generate_voice(text, emo)`: 生成语音
- `get_params()`: 获取当前参数

### SherpaOnnxManager

模型管理器，负责模型的下载、验证和管理。

#### 方法

- `initialize()`: 初始化管理器
- `get_models()`: 获取所有模型
- `add_model(model_info)`: 添加模型
- `remove_model(model_name)`: 删除模型
- `validate_model(model_name)`: 验证模型

## 更新和维护

### 更新模型

定期检查并更新模型：

```bash
# 删除旧模型
node scripts/download_sherpa_onnx_models.mjs --delete vits-zh

# 下载新版本
node scripts/download_sherpa_onnx_models.mjs vits-zh
```

### 清理空间

清理不需要的模型文件：

```bash
# 查看存储使用情况
node scripts/download_sherpa_onnx_models.mjs --list

# 删除不需要的模型
rm -rf data/sherpa_onnx_models/old-model
```

## 贡献

欢迎提交问题报告和功能请求！

### 开发环境设置

1. 克隆仓库
2. 安装依赖：`pnpm install`
3. 运行测试：`pnpm test`
4. 提交更改

### 代码规范

- 遵循 Rust 代码规范
- 添加适当的测试
- 更新相关文档

## 许可证

Sherpa-ONnx 遵循 Apache 2.0 许可证。详见 LICENSE 文件。
