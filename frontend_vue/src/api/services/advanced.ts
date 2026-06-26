import http from '../http'

export interface MemoryNote {
  id: string
  content: string
  tags?: string[]
  source?: string
  createdAt?: string
}

export interface AdvancedFeaturesData {
  memoryNotes?: MemoryNote[]
}

export const getAdvancedFeatures = async (): Promise<AdvancedFeaturesData> => {
  try {
    const data = await http.get('/v1/chat/advanced/get')
    return data
  } catch (error: any) {
    console.error('获取高级功能数据错误:', error.message)
    throw error
  }
}

export const saveAdvancedFeatures = async (data: AdvancedFeaturesData): Promise<void> => {
  try {
    await http.post('/v1/chat/advanced/save', data)
    // 触发更新事件
    window.dispatchEvent(new CustomEvent('advanced-features-updated'))
  } catch (error: any) {
    console.error('保存高级功能数据错误:', error.message)
    throw error
  }
}
