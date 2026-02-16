import http from '../http'
import type { Character, CharacterSelectParams } from '../../types'

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

export interface RoleInfo {
  character_id: number
  ai_name: string
  ai_subtitle: string
  thinking_message: string
  scale: number
  offset_x: number
  offset_y: number
  bubble_top: number
  bubble_left: number
  clothes: object
  clothes_name: string
  body_part: object
}

export const getRoleInfo = async (roleId: number): Promise<RoleInfo> => {
  try {
    const data = await http.get(`/v1/chat/character/get_role_info/${roleId}`)
    console.log('获取角色信息成功', data)
    return data
  } catch (error: any) {
    console.error('获取游戏角色信息错误:', error.message)
    throw error
  }
}

export const getRoleSettings = async (roleId: number): Promise<any> => {
  try {
    const data = await http.get(`/v1/chat/character/get_full_role_settings/${roleId}`)
    return data
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || '获取角色配置失败')
  }
}

export const updateRoleSettings = async (roleId: number, settings: any): Promise<any> => {
  try {
    const response = await http.post('/v1/chat/character/update_settings', {
      role_id: roleId,
      settings: settings,
    })
    return response
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || '更新角色配置失败')
  }
}
