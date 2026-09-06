# Sherpa-ONNX 集成指南

本文档介绍了如何在 LingChat 中集成 Sherpa-ONNX 作为内置语音引擎。

## 功能特性

- 🎤 **多种模型支持**：支持 VITS、FastSpeech2、Tortoise、Matcha-TTS 等多种模型
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
  "sherpa_onnx_model_path": "data/tts-local/sherpa_onnx_models/vits-zh",
  "sherpa_onnx_model_type": "vits",
  "sherpa_onnx_lang": "zh",
  "sherpa_onnx_voice": "female",
  "sherpa_onnx_use_gpu": false
}
```

### 3. 使用 Sherpa-ONNX

在角色设置中选择 "sherpa-onnx" 作为 TTS 类型即可开始使用。

## 支持的模型

### VITS 模型

- **vits-zh**: 中文 VITS 模型（基于 AIShell3 数据集）
- **vits-en**: 英文 VITS 模型（基于 LJSpeech 数据集）

### FastSpeech2 模型

- **fastspeech2-zh**: 中文 FastSpeech2 模型
- **fastspeech2-en**: 英文 FastSpeech2 模型

### Tortoise 模型

- **tortoise-en**: 英文 Tortoise 模型（高质量语音合成）

### Matcha-TTS 模型

- **matcha-zh**: 中文 Matcha-TTS 模型
- **matcha-en**: 英文 Matcha-TTS 模型

## 配置选项

### 基础配置

| 参数                     | 类型    | 默认值   | 描述              |
| ------------------------ | ------- | -------- | ----------------- |
| `sherpa_onnx_model_name` | String  | -        | 模型名称          |
| `sherpa_onnx_model_path` | String  | -        | 模型文件路径      |
| `sherpa_onnx_model_type` | String  | "vits"   | 模型类型          |
| `sherpa_onnx_lang`       | String  | "zh"     | 语言代码          |
| `sherpa_onnx_voice`      | String  | "female" | 音色名称          |
| `sherpa_onnx_use_gpu`    | Boolean | false    | 是否使用 GPU 加速 |

### 模型特定参数

#### VITS 模型

- `speed`: 语速（默认：1.0）
- `noise_scale`: 噪声尺度（默认：0.667）
- `noise_scale_w`: 噪声尺度 W（默认：0.8）

#### FastSpeech2 模型

- `duration_scale`: 时长缩放（默认：1.0）
- `energy_scale`: 能量缩放（默认：1.0）
- `pitch_scale`: 音高缩放（默认：1.0）

#### Tortoise 模型

- `temperature`: 温度参数（默认：0.8）
- `diffusion_temperature`: 扩散温度（默认：1.0）

## 高级用法

### 自定义模型

1. 下载自定义模型文件到 `data/tts-local/sherpa_onnx_models/your-model-name/`
2. 确保 `model.onnx` 文件存在
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

支持多种 GPU 后端：

```json
{
  "sherpa_onnx_use_gpu": true,
  "sherpa_onnx_device": "cuda" // 或 "directml" (Windows)
}
```

### 批量处理

```javascript
// 批量下载多个模型
const models = ["vits-zh", "fastspeech2-en", "tortoise-en"];
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
rm -rf data/tts-local/sherpa_onnx_models/old-model
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
