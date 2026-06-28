import http from '../http'

export interface AmbientItem {
  name: string
  url: string
}

export const ambientGetAll = async (): Promise<AmbientItem[]> => {
  try {
    const data = await http.get('/v1/chat/ambient/list')
    return data
  } catch (error: any) {
    throw new Error(error.response?.data?.message || 'Failed to load ambient list')
  }
}

export const ambientUpload = async (formData: FormData): Promise<void> => {
  try {
    await http.post('/v1/chat/ambient/upload', formData)
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || 'Ambient upload failed')
  }
}

export const ambientDelete = async (url: string): Promise<void> => {
  try {
    await http.delete('/v1/chat/ambient/delete', {
      params: { url },
    })
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || 'Ambient delete failed')
  }
}
