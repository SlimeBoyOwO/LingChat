import http from '../http'

/** 配置方案摘要 */
export interface LlmConfigSummary {
  name: string
  display_name: string
  description: string
  is_active: boolean
  main_provider?: string
}

/** 单段模型配置（main / translator） */
export interface LlmModelSection {
  provider: string
  model: string
  api_key: string
  base_url: string
  proxy?: string
  temperature?: number | string
  top_p?: number | string
  enable_thinking?: string
}

/** 完整配置方案 */
export interface LlmConfigScheme {
  config_name?: string
  config_description?: string
  main: LlmModelSection
  translator: LlmModelSection
  providers?: Record<string, any>
}

/** 活跃配置详情 */
export interface ActiveConfigResponse {
  status: string
  name: string
  config: LlmConfigScheme
}

/** 配置方案列表响应 */
export interface ConfigsListResponse {
  status: string
  data: LlmConfigSummary[]
}

/** 测试请求 */
export interface TestProviderRequest {
  provider: string
  model: string
  api_key: string
  base_url: string
  proxy?: string
  temperature?: number | null
  top_p?: number | null
  enable_thinking?: string
  message?: string
}

/** 测试结果 */
export interface TestProviderResponse {
  status: string
  response: string
}

/** 列出所有配置方案 */
export async function listConfigs(): Promise<LlmConfigSummary[]> {
  const res = await http.get<any>('/v1/llm-config/configs')
  return res.data || res
}

/** 获取当前激活配置详情 */
export async function getActiveConfig(): Promise<ActiveConfigResponse> {
  return http.get('/v1/llm-config/active')
}

/** 切换激活配置 */
export async function switchConfig(name: string): Promise<{ status: string; message: string; active: string }> {
  return http.post('/v1/llm-config/switch', { name })
}

/** 保存/更新配置方案 */
export async function saveConfig(
  name: string,
  config: LlmConfigScheme,
): Promise<{ status: string; message: string; saved: string }> {
  return http.post('/v1/llm-config/save', { name, config })
}

/** 删除配置方案 */
export async function deleteConfig(name: string): Promise<{ status: string; message: string; deleted: string }> {
  return http.delete(`/v1/llm-config/${encodeURIComponent(name)}`)
}

/** 获取指定配置方案的完整内容 */
export async function getConfig(name: string): Promise<{ name: string; config: LlmConfigScheme }> {
  return http.get(`/v1/llm-config/config/${encodeURIComponent(name)}`)
}

/** 测试 LLM 提供商 */
export async function testProvider(data: TestProviderRequest): Promise<TestProviderResponse> {
  return http.post('/v1/llm-config/test', data)
}
