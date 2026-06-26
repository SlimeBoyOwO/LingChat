import http from '../http'

export type StructuredConfig = Record<string, any>

// 单个配置项的类型
export interface ConfigItem {
  key: string
  value: string
  description: string
  type: 'text' | 'bool' | 'textarea'
}

// 保存请求的负载类型（键值对）
interface SavePayload {
  [key: string]: string
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
  // 从已有的 /settings 接口获取所有配置，再提取目标 key
  const allSettings = await getEnvConfigSettings()
  for (const category of Object.values(allSettings)) {
    const cat = category as any
    const subcategories = cat?.subcategories
    if (!subcategories) continue
    for (const sub of Object.values(subcategories) as any[]) {
      for (const setting of sub?.settings || []) {
        if (setting.key === key) {
          return setting as ConfigItem
        }
      }
    }
  }
  throw new Error(`配置项 ${key} 未找到`)
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
