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

export const musicUpload = async (path: string, fileName: string): Promise<void> => {
  try {
    await invoke('upload_music', { path, fileName })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : error.message || 'Music upload failed')
  }
}

export const musicDelete = async (url: string): Promise<void> => {
  try {
    await invoke('delete_music', { url })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : error.message || 'Music delete failed')
  }
}

export const setCurrentBackgroundMusic = async (music: string): Promise<void> => {
  await http.post('/v1/chat/back-music/select', { music })
}

/** 持久化背景音乐状态到 settings.json，下次启动时自动恢复 */
export const saveBgmState = async (
  track: string,
  paused: boolean,
  mode: string,
): Promise<void> => {
  try {
    await invoke('save_bgm_state', { track, paused, mode })
  } catch (error: any) {
    console.warn('持久化BGM状态失败（非致命）:', typeof error === 'string' ? error : error.message)
  }
}
