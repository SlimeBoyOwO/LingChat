import { invoke } from '@tauri-apps/api/core'

export type StructuredConfig = Record<string, any>

// 单个配置项的类型
export interface ConfigItem {
  key: string
  value: string
  description: string
  type: 'text' | 'bool' | 'textarea' | 'path' | 'number'
}

export async function fetchEnvConfig(): Promise<StructuredConfig> {
  return invoke('get_settings_tree')
}

export async function saveEnvConfig(
  values: Record<string, string>,
): Promise<string> {
  return invoke('save_settings', { values })
}

/**
 * 设置 HDR 模式开关（仅 Windows）。
 * 持久化到后端 settings.json，启动时由 Rust 侧读取并决定 WebView2 色彩配置，重启后生效。
 */
export async function setHdrMode(enabled: boolean): Promise<void> {
  return invoke('set_hdr_mode', { enabled })
}

export const getEnvConfigByKey = async (key: string): Promise<ConfigItem> => {
  try {
    const data = await invoke('get_setting_by_key', { key })
    return data as ConfigItem
  } catch (error) {
    console.error('Error fetching config by key:', error)
    throw error
  }
}

export const getEnvConfigSettings = async (): Promise<StructuredConfig> => {
  try {
    const data = await invoke('get_settings_tree')
    return data as StructuredConfig
  } catch (error) {
    console.error('Error fetching config env settings:', error)
    throw error
  }
}

export const saveEnvConfigSettings = async (
  values: Record<string, string>,
): Promise<{ status: string; message: string }> => {
  try {
    const message = await invoke('save_settings', { values })
    return { status: 'success', message: message as string }
  } catch (error) {
    console.error('Error modifying config env settings:', error)
    throw error
  }
}

// 记忆嵌入运行状态（高级设置 → 记忆嵌入 界面诊断展示）
export interface EmbeddingStatus {
  enabled: boolean
  configured: boolean
  ready: boolean
  dim: number | null
  model: string | null
  modelDir: string
  backend: string
  defaultModelDir: string
  error: string | null
  indexLen: number
}

export async function getEmbeddingStatus(): Promise<EmbeddingStatus> {
  const data = await invoke('get_embedding_status')
  return data as EmbeddingStatus
}

// 独立语义记忆运行状态（高级设置 → 语义记忆 界面诊断展示）
export interface SemanticMemoryStatus {
  enabled: boolean
  opened: boolean
  embeddingReady: boolean
  dbPath: string
  count: number
  error: string | null
}

export async function getSemanticMemoryStatus(): Promise<SemanticMemoryStatus> {
  const data = await invoke('get_semantic_memory_status')
  return data as SemanticMemoryStatus
}

// ---------- 语义记忆可视化管理 ----------

export interface SemanticMemoryRole {
  id: number
  name: string
  roleType: string
  isCurrent: boolean
}

export interface SemanticMemoryItem {
  id: string
  text: string
  tags: string[]
  createdAt: string
}

export interface SemanticMemoryWriteResult {
  ok: boolean
  id: string | null
  outcome: string
}

export async function listSemanticMemoryRoles(): Promise<SemanticMemoryRole[]> {
  const data = await invoke('list_semantic_memory_roles')
  return data as SemanticMemoryRole[]
}

export async function listSemanticMemories(roleId: number): Promise<SemanticMemoryItem[]> {
  const data = await invoke('list_semantic_memories', { roleId })
  return data as SemanticMemoryItem[]
}

export async function addSemanticMemory(
  roleId: number,
  content: string,
  tags: string[],
): Promise<SemanticMemoryWriteResult> {
  return invoke('add_semantic_memory', {
    roleId,
    content,
    tags,
  }) as Promise<SemanticMemoryWriteResult>
}

export async function updateSemanticMemory(
  roleId: number,
  id: string,
  content: string,
): Promise<SemanticMemoryWriteResult> {
  return invoke('update_semantic_memory', {
    roleId,
    id,
    content,
  }) as Promise<SemanticMemoryWriteResult>
}

export async function deleteSemanticMemory(
  roleId: number,
  id: string,
): Promise<SemanticMemoryWriteResult> {
  return invoke('delete_semantic_memory', { roleId, id }) as Promise<SemanticMemoryWriteResult>
}
