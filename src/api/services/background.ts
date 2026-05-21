import { invoke } from '@tauri-apps/api/core'
import http from '../http'
import type { BackgroundImageInfo } from '../../types'

export const getBackgroundImages = async (): Promise<BackgroundImageInfo[]> => {
  try {
    const data = await invoke('get_background_list')
    return data as BackgroundImageInfo[]
  } catch (error: any) {
    console.error(
      'Failed to get background list:',
      typeof error === 'string' ? error : error.message,
    )
    throw error
  }
}

export const getBackgroundImageById = async (id: string): Promise<BackgroundImageInfo> => {
  return http.get(`/backgrounds/${id}`)
}

export const uploadBackgroundImage = async (
  fileName: string,
  fileData: Uint8Array,
): Promise<BackgroundImageInfo[]> => {
  return invoke('upload_background_image', { fileName, fileData })
}

export const setCurrentBackground = async (background: string): Promise<void> => {
  await http.post('/v1/chat/background/select', { background })
}

export const setCurrentBackgroundEffect = async (effect: string): Promise<void> => {
  await http.post('/v1/chat/background/effect', { effect })
}

export const generateBackgroundImage = async (prompt: string, clientId: string): Promise<void> => {
  await http.post('/v1/chat/background/generate', {
    prompt,
    client_id: clientId,
  })
}

export const openBackgroundsFolder = async (): Promise<void> => {
  await invoke('open_backgrounds_folder')
}
