import { invoke } from '@tauri-apps/api/core'

export type StructuredConfig = Record<string, any>

// 单个配置项的类型
export interface ConfigItem {
  key: string
  value: string
  description: string
  type: 'text' | 'bool' | 'textarea' | 'path'
}

export async function fetchEnvConfig(): Promise<StructuredConfig> {
  return invoke('get_settings_tree')
}

export async function saveEnvConfig(
  values: Record<string, string>,
): Promise<string> {
  return invoke('save_settings', { values })
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
