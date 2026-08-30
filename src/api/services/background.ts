import { invoke } from '@tauri-apps/api/core'
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

export const uploadBackgroundImage = async (
  fileName: string,
  fileData: Uint8Array,
): Promise<BackgroundImageInfo[]> => {
  return invoke('upload_background_image', { fileName, fileData })
}

export const openBackgroundsFolder = async (): Promise<void> => {
  await invoke('open_backgrounds_folder')
}
