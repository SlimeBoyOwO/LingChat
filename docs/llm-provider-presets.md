# LLM 提供商预设（开发者修改指南）

## 概述

「预设」是 LLM 提供商表单里的**快速配置**按钮：点击某个预设，会自动把表单的
名称 / 提供商 / 模型 / API 地址填充为对应配置，方便用户快速接入常用服务商。

预设**不再写死在前端组件里**，而是统一放在独立文件：

- 数据定义：`src/constants/llm-presets.ts`
- 使用位置：`src/components/settings/pages/SettingsLlmProviders.vue`（遍历渲染预设按钮）

## 如何新增 / 修改 / 删除预设

只需编辑 `src/constants/llm-presets.ts` 中的 `llmPresets` 数组，前端会自动生效，**无需改动组件代码**。

每个预设是一个对象：

```ts
{
  key: '唯一标识',          // 用作按钮 key，需保持唯一
  label: '按钮显示名',      // 例如 'DeepSeek V4 Flash'
  provider: 'openai',       // 提供商类型，需与后端 provider 解析逻辑一致
  model: '模型名',          // 默认模型名，可留空字符串
  base_url: 'https://api.example.com/v1',  // API 地址
}
```

### 新增

在 `llmPresets` 数组末尾追加一个对象即可，例如：

```ts
{
  key: 'my-provider',
  label: '我的服务商',
  provider: 'openai',
  model: 'my-model',
  base_url: 'https://api.my-provider.com/v1',
},
```

### 修改

直接改对应对象的 `label` / `provider` / `model` / `base_url` 字段。

### 删除

移除对应的对象（同时保证 `key` 在剩余预设中仍唯一）。

## 字段约定

| 字段 | 说明 |
|------|------|
| `key` | 唯一标识（按钮 key），建议小写连字符风格，如 `deepseek-v4-flash` |
| `label` | 按钮显示名（可含中文，会直接展示给用户） |
| `provider` | 提供商类型。`openai` 兼容协议用 `openai`；LM Studio 等本地推理用 `lmstudio`。需与后端解析逻辑匹配，否则连接会失败 |
| `model` | 默认模型名。本地推理（如 Ollama / LM Studio）可留空，让用户自行填写 |
| `base_url` | API 地址。注意：纯前端预设，URL 不会加密存储，请勿写入私有密钥 |

## 注意事项

- **`provider` 必须与后端支持的类型一致**：改错会导致「连接测试」/实际请求失败。
- **`key` 必须唯一**：重复会导致按钮渲染异常。
- 预设只是**填充表单**，不会自动保存或测试连接；用户仍需点击「保存」。
- 若需支持新的 `provider` 类型（除 openai / lmstudio 外），需要同时在 Rust 侧
  `src-tauri/src/ai_service/llm/provider_config.rs`（`build_llm_client_from_provider`）
  增加对应分支，否则前端填了也连不上。
