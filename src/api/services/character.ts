import { invoke } from '@tauri-apps/api/core'
import http from '../http'
import type { Character, CharacterSelectParams } from '../../types'
import type { WebInitData } from './game-info'

interface CharacterSelectResponse {
  success: boolean
  character: {
    id: number
    title: string
    folder_name: string
  }
}

export interface CharacterPageResult {
  items: Character[]
  total: number
  page: number
  page_size: number
  total_pages: number
}

export const characterGetAll = async (
  page: number = 1,
  pageSize: number = 6,
): Promise<CharacterPageResult> => {
  try {
    const data = await invoke('get_character_list', { page, pageSize })
    return data as CharacterPageResult
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : '获取角色列表失败')
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

/** 切换角色并返回最新游戏初始化数据 */
export const selectCharacter = async (characterId: number): Promise<WebInitData> => {
  try {
    const data = await invoke<WebInitData>('select_character', { characterId })
    return data
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : '角色切换失败')
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
  scale_p: number
  offset_x_p: number
  offset_y_p: number
  bubble_top: number
  bubble_left: number
  clothes: object
  clothes_name: string
  body_part: object
  character_folder: string
}

export const getRoleInfo = async (roleId: number): Promise<RoleInfo> => {
  try {
    const data = await invoke('get_role_info', { roleId })
    console.log('获取角色信息成功', data)
    return data as RoleInfo
  } catch (error: any) {
    console.error('获取游戏角色信息错误:', typeof error === 'string' ? error : error.message)
    throw error
  }
}

export const getRoleSettings = async (roleId: number): Promise<any> => {
  try {
    return await invoke('get_role_settings', { roleId })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : '获取角色配置失败')
  }
}

export const updateRoleSettings = async (roleId: number, settings: any): Promise<any> => {
  try {
    return await invoke('update_role_settings', { roleId, settings })
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : '更新角色配置失败')
  }
}

export interface CreateCharacterResponse {
  success: boolean
  data: {
    character_id: number
    title: string
    resource_folder: string
  }
}

export const createCharacter = async (formData: FormData): Promise<CreateCharacterResponse> => {
  try {
    const response = await http.post('/v1/chat/character/create', formData)
    return response
  } catch (error: any) {
    throw new Error(error.response?.data?.detail || '创建角色失败')
  }
}

export interface SelectClothesResponse {
  success: boolean
  message: string
}

export const selectClothes = async (clothesName: string): Promise<SelectClothesResponse> => {
  try {
    const data = await invoke('select_clothes', { clothesName })
    return data as SelectClothesResponse
  } catch (error: any) {
    throw new Error(typeof error === 'string' ? error : '选择衣服失败')
  }
}

/** 获取角色资源文件的绝对路径（供 convertFileSrc 使用） */
export const getCharacterFilePath = async (filePath: string): Promise<string> => {
  return invoke('get_character_file', { filePath })
}

export const getAvatarFile = async (
  characterFolder: string,
  clothesName: string,
): Promise<string> => {
  return invoke('get_avatar_file', { characterFolder, emotion: '头像', clothesName })
}
