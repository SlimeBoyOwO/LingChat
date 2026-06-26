import { invoke } from '@tauri-apps/api/core'

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

export const ambientUpload = async (fileName: string, fileData: Uint8Array): Promise<void> => {
  try {
    await invoke('upload_ambient', { fileName, fileData })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : error.message || '环境音上传失败')
  }
}

export const ambientDelete = async (url: string): Promise<void> => {
  try {
    await invoke('delete_ambient', { url })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : error.message || '环境音删除失败')
  }
}
