import { invoke } from '@tauri-apps/api/core'
import { i18n } from '@/locales'

// ========== 聊天音效数据模型 ==========

export interface ChatSoundItem {
  name: string
  url: string
}

// ========== 聊天音效服务 ==========

export const chatSoundGetAll = async (): Promise<ChatSoundItem[]> => {
  try {
    const data = await invoke('get_chat_sound_list')
    return data as ChatSoundItem[]
  } catch (error: any) {
    console.error('获取聊天音效列表失败:', typeof error === 'string' ? error : error.message)
    throw error
  }
}

export const chatSoundUpload = async (path: string, fileName: string): Promise<void> => {
  try {
    await invoke('upload_chat_sound', { path, fileName })
  } catch (error: any) {
    throw new Error(
      typeof error === 'string' ? error : error.message || i18n.global.t('api.chatSound.uploadFailed'),
    )
  }
}

export const chatSoundDelete = async (url: string): Promise<void> => {
  try {
    await invoke('delete_chat_sound', { url })
  } catch (error: any) {
    throw new Error(
      typeof error === 'string' ? error : error.message || i18n.global.t('api.chatSound.deleteFailed'),
    )
  }
}
