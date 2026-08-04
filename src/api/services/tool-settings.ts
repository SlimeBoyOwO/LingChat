import { invoke } from '@tauri-apps/api/core'
import { ref } from 'vue'

/** 网页搜索工具配置（与后端 WebSearchSettings 对应，字段保持 snake_case）。 */
export interface WebSearchSettings {
  enabled: boolean
  /** true = 模型 API 内置联网（免 Key）；false = 独立搜索端点 + api_key */
  use_builtin: boolean
  api_key: string
  base_url: string
  proxy_enabled: boolean
  proxy_addr: string
  max_results: number
  /** true = 搜索结果不带来源/网址，模型回答中不显示原始搜索结果 */
  hide_search_results: boolean
}

export interface ToolSettings {
  web_search: WebSearchSettings
}

/** 后端 `ai:tool_call` 事件的载荷 + 前端补充的时间戳。 */
export interface ToolCallRecord {
  tool: string
  ok: boolean
  summary: string
  error: string | null
  time: string
}

const MAX_HISTORY = 20

/** 最近的工具调用记录（内存态，最新在前），供「工具调用」页面展示。 */
export const recentToolCalls = ref<ToolCallRecord[]>([])

export function pushToolCallRecord(record: ToolCallRecord) {
  recentToolCalls.value.unshift(record)
  if (recentToolCalls.value.length > MAX_HISTORY) {
    recentToolCalls.value.length = MAX_HISTORY
  }
}

export function getToolSettings(): Promise<ToolSettings> {
  return invoke<ToolSettings>('get_tool_settings')
}

export function saveToolSettings(settings: ToolSettings): Promise<void> {
  return invoke<void>('save_tool_settings', { settings })
}

/** 直接执行一次网页搜索；失败时 Promise reject 携带后端错误信息。 */
export function testWebSearch(query: string): Promise<string> {
  return invoke<string>('test_web_search', { query })
}
