import { invoke } from '@tauri-apps/api/core'

export interface LlmProviderConfig {
  id: string
  label: string
  provider: string
  model: string
  api_key: string
  base_url: string
  temperature: number | null
  top_p: number | null
  enable_thinking: boolean
  reasoning_effort: string | null
}

export interface LlmProvidersResponse {
  providers: LlmProviderConfig[]
  chat_provider_id: string | null
  translate_provider_id: string | null
  god_agent_provider_id: string | null
  vision_provider_id: string | null
}

export interface LlmModelInfo {
  id: string
  display_name: string | null
  context_length: number | null
  supports_reasoning: boolean
  supports_thinking_type: string | null
  think_efforts: {
    valid_efforts: string[]
    default_effort: string | null
  } | null
}

export async function listLlmProviders(): Promise<LlmProvidersResponse> {
  return invoke('list_llm_providers')
}

export async function saveLlmProvider(provider: LlmProviderConfig): Promise<void> {
  return invoke('save_llm_provider', { provider })
}

export async function deleteLlmProvider(id: string): Promise<void> {
  return invoke('delete_llm_provider', { id })
}

export async function setLlmRole(
  role: 'chat' | 'translate' | 'god_agent' | 'vision',
  providerId: string | null,
): Promise<void> {
  return invoke('set_llm_role', { role, providerId })
}

export async function listLlmModels(
  provider: LlmProviderConfig,
): Promise<LlmModelInfo[]> {
  return invoke('list_llm_models', { provider })
}

/** 热切换 LLM —— 重建所有角色的 LLM 客户端，无需重启应用。 */
export async function switchLlm(): Promise<void> {
  return invoke('switch_llm')
}
