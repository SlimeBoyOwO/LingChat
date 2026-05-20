import { invoke } from '@tauri-apps/api/core'
import http from '../http'
import type { MusicTrack } from '../../types'

export const musicGetAll = async (): Promise<MusicTrack[]> => {
  try {
    const data = await invoke('get_music_list')
    return data as MusicTrack[]
  } catch (error: any) {
    console.error('Failed to get music list:', typeof error === 'string' ? error : error.message)
    throw error
  }
}

export const musicUpload = async (formData: FormData): Promise<void> => {
  try {
    await http.post('/v1/chat/back-music/upload', formData)
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || 'Music upload failed')
  }
}

export const musicDelete = async (url: string): Promise<void> => {
  try {
    await http.delete('/v1/chat/back-music/delete', {
      params: { url: url },
    })
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || 'Music delete failed')
  }
}

export const setCurrentBackgroundMusic = async (music: string): Promise<void> => {
  await http.post('/v1/chat/back-music/select', { music })
}
