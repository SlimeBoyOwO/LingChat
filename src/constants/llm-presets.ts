/**
 * LLM 提供商预设（快速配置）。
 *
 * 新增/修改预设只需编辑本文件 —— 前端 `SettingsLlmProviders.vue` 会遍历本数组自动渲染
 * 出预设按钮，点击即可把表单字段填充为对应配置，无需改动组件代码。
 *
 * 开发者修改指南见：docs/llm-provider-presets.md
 *
 * 字段说明：
 * - key:      唯一标识（用作按钮的 key，需保持唯一）
 * - label:    按钮显示名
 * - provider: 提供商类型（如 openai / lmstudio，需与后端 provider 解析逻辑一致）
 * - model:    默认模型名（可为空字符串，用户后续可改）
 * - base_url: API 地址
 */
export interface LlmPreset {
  key: string
  label: string
  provider: string
  model: string
  base_url: string
}

export const llmPresets: LlmPreset[] = [
  {
    key: 'deepseek-v4-flash',
    label: 'DeepSeek V4 Flash',
    provider: 'openai',
    model: 'deepseek-v4-flash',
    base_url: 'https://api.deepseek.com',
  },
  {
    key: 'deepseek-v4-pro',
    label: 'DeepSeek V4 Pro',
    provider: 'openai',
    model: 'deepseek-v4-pro',
    base_url: 'https://api.deepseek.com',
  },
  {
    key: 'qwen-max',
    label: '通义千问 Max',
    provider: 'openai',
    model: 'qwen3.7-max',
    base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
  },
  {
    key: 'qwen-plus',
    label: '通义千问 Plus',
    provider: 'openai',
    model: 'qwen3.7-plus',
    base_url: 'https://dashscope.aliyuncs.com/compatible-mode/v1',
  },
  {
    key: 'kimi',
    label: 'Kimi K2.6',
    provider: 'openai',
    model: 'kimi-k2.6',
    base_url: 'https://api.moonshot.cn/v1',
  },
  {
    key: 'ollama',
    label: 'Ollama',
    provider: 'openai',
    model: '',
    base_url: 'http://localhost:11434/v1',
  },
  {
    key: 'lmstudio',
    label: 'LM Studio',
    provider: 'lmstudio',
    model: '',
    base_url: 'http://localhost:1234/v1',
  },
  {
    key: 'codex',
    label: 'OpenAI Codex',
    provider: 'codex',
    model: 'gpt-5.6-sol',
    base_url: '',
  },
]
