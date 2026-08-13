import { invoke } from '@tauri-apps/api/core'
import { i18n } from '@/locales'

// ========== 环境音数据模型 ==========
export interface AmbientItem {
  name: string
  url: string
}

// ========== 环境音服务 ==========

export const ambientGetAll = async (): Promise<AmbientItem[]> => {
  try {
    const data = await invoke('get_ambient_list')
    return data as AmbientItem[]
  } catch (error: any) {
    console.error('获取环境音列表失败:', typeof error === 'string' ? error : error.message)
    throw error
  }
}

export const ambientUpload = async (path: string, fileName: string): Promise<void> => {
  try {
    await invoke('upload_ambient', { path, fileName })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : error.message || i18n.global.t('api.ambient.uploadFailed'))
  }
}

export const ambientDelete = async (url: string): Promise<void> => {
  try {
    await invoke('delete_ambient', { url })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : error.message || i18n.global.t('api.ambient.deleteFailed'))
  }
}

/** 持久化环境音轨道列表到 settings.json，下次启动时自动恢复 */
export const saveAmbientState = async (tracksJson: string): Promise<void> => {
  try {
    await invoke('save_ambient_state', { tracksJson })
  } catch (error: any) {
    console.warn('持久化环境音状态失败（非致命）:', typeof error === 'string' ? error : error.message)
  }
}
