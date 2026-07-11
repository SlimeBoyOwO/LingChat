import { invoke } from '@tauri-apps/api/core'

export type StructuredConfig = Record<string, any>

// 单个配置项的类型
export interface ConfigItem {
  key: string
  value: string
  description: string
  type: 'text' | 'bool' | 'textarea' | 'path' | 'select'
  options?: string[]
}

export interface WindowDimensions {
  width: number
  height: number
}

export interface WindowSaveResult {
  status: 'applied' | 'deferred'
  requested: WindowDimensions
  applied: WindowDimensions
  adjusted: boolean
}

export interface SaveSettingsResult {
  status: string
  message: string
  window?: WindowSaveResult
}

const normalizeSaveSettingsResult = (
  result: string | SaveSettingsResult,
): SaveSettingsResult => {
  // 兼容仍返回纯字符串的旧版 Rust 后端。
  if (typeof result === 'string') {
    return { status: 'success', message: result }
  }
  return result
}

export async function fetchEnvConfig(): Promise<StructuredConfig> {
  return invoke('get_settings_tree')
}

export async function saveEnvConfig(
  values: Record<string, string>,
): Promise<SaveSettingsResult> {
  const result = await invoke<string | SaveSettingsResult>('save_settings', { values })
  return normalizeSaveSettingsResult(result)
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
): Promise<SaveSettingsResult> => {
  try {
    const result = await invoke<string | SaveSettingsResult>('save_settings', { values })
    return normalizeSaveSettingsResult(result)
  } catch (error) {
    console.error('Error modifying config env settings:', error)
    throw error
  }
}
