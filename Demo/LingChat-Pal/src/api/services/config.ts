import http from '../http'

export type StructuredConfig = Record<string, any>

// 单个配置项的类型
export interface ConfigItem {
  key: string
  value: string
  description: string
  type: 'text' | 'bool' | 'textarea'
}

export async function fetchEnvConfig(): Promise<StructuredConfig> {
  return http.get('/settings/config')
}

export async function saveEnvConfig(
  values: Record<string, string>,
): Promise<{ status: string; message: string }> {
  return http.post('/settings/config', values)
}

export const getEnvConfigByKey = async (key: string): Promise<ConfigItem> => {
  try {
    const data = await http.get(`/v1/chat/config/key/${key}`)
    return data as ConfigItem
  } catch (error) {
    console.error('Error fetching config by key:', error)
    throw error
  }
}

export const getEnvConfigSettings = async (): Promise<StructuredConfig> => {
  try {
    const data = await http.get(`/v1/chat/config/settings`)
    return data
  } catch (error) {
    console.error('Error fetching config env settings:', error)
    throw error
  }
}

export const saveEnvConfigSettings = async (
  values: Record<string, string>,
): Promise<{ status: string; message: string }> => {
  try {
    const data = await http.patch(`/v1/chat/config/settings`, values)
    return data
  } catch (error) {
    console.error('Error modifying config env settings:', error)
    throw error
  }
}
