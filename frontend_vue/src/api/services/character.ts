import http from '../http'
import type { Character, CharacterSelectParams, Clothes } from '../../types'

// 角色选择API返回类型
interface CharacterSelectResponse {
  success: boolean
  character: {
    id: number
    title: string
    folder_name: string // 角色文件夹名，用于加载专属提示
  }
}

export const characterGetAll = async (): Promise<Character[]> => {
  try {
    const data = await http.get('/v1/chat/character/get_all_characters')
    return data
  } catch (error: any) {
    throw new Error(error.response?.data?.message || '获取角色列表失败')
  }
}

export const characterSelect = async (
  params: CharacterSelectParams,
): Promise<CharacterSelectResponse> => {
  try {
    const response = await http.post('/v1/chat/character/select_character', params)
    return response
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || '角色选择失败')
  }
}
