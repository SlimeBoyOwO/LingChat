import http from '../http'
import type { BackgroundImageInfo } from '../../types'

export const getBackgroundImages = async (): Promise<BackgroundImageInfo[]> => {
  try {
    const data = await http.get('/v1/chat/background/list', {})
    return data
  } catch (error: any) {
    console.error('Failed to get background list:', error.message)
    throw error
  }
}

export const getBackgroundImageById = async (id: string): Promise<BackgroundImageInfo> => {
  return http.get(`/backgrounds/${id}`)
}

export const uploadBackgroundImage = async (file: File): Promise<BackgroundImageInfo> => {
  const formData = new FormData()
  formData.append('file', file)
  return http.post('/backgrounds/upload', formData, {
    headers: {
      'Content-Type': 'multipart/form-data',
    },
  })
}

export const setCurrentBackground = async (background: string): Promise<void> => {
  await http.post('/v1/chat/background/select', { background })
}

export const setCurrentBackgroundEffect = async (effect: string): Promise<void> => {
  await http.post('/v1/chat/background/effect', { effect })
}
