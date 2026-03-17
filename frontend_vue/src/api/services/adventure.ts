import http from '@/api/http'

export interface AdventureInfo {
  adventure_folder: string
  name: string
  description: string
  order: number
  status: 'locked' | 'unlocked' | 'in_progress' | 'completed'
  unlocked_at?: string
  completed_at?: string
  unlock_conditions?: Array<{
    type: string
    threshold?: number
    start_hour?: number
    end_hour?: number
    adventure_folder?: string
    achievement_id?: string
  }>
  trigger?: {
    mode: 'manual' | 'auto_random' | 'auto_immediate'
    random_chance?: number
  }
}

export interface AdventureProgress {
  adventure_folder: string
  character_folder: string
  status: string
  unlocked_at?: string
  completed_at?: string
  progress_data?: Record<string, any>
}

export interface UnlockedAdventure {
  adventure_folder: string
  name: string
  description: string
  character_folder: string
  order: number
}

/**
 * 获取指定角色的所有羁绊冒险列表（含解锁状态）
 */
export const getCharacterAdventures = async (
  characterFolder: string,
  userId: number = 1,
): Promise<AdventureInfo[]> => {
  const encodedFolder = encodeURIComponent(characterFolder)
  const url = `/v1/chat/adventure/list/${encodedFolder}?user_id=${userId}`
  console.log('[AdventureAPI] Fetching adventures:', { characterFolder, encodedFolder, url })
  const response = await http.get(url)
  console.log('[AdventureAPI] Response:', response)
  // 响应拦截器已经提取了 data，所以 response 就是数组
  return Array.isArray(response) ? response : []
}

/**
 * 获取所有羁绊冒险（含解锁状态）
 */
export const getAllAdventures = async (userId: number = 1): Promise<AdventureInfo[]> => {
  const response = await http.get(`/v1/chat/adventure/all?user_id=${userId}`)
  return Array.isArray(response) ? response : []
}

/**
 * 获取用户的全部冒险进度
 */
export const getUserProgress = async (userId: number): Promise<AdventureProgress[]> => {
  const response = await http.get(`/v1/chat/adventure/progress/${userId}`)
  return Array.isArray(response) ? response : []
}

/**
 * 启动指定羁绊冒险
 */
export const startAdventure = async (userId: number, adventureFolder: string): Promise<any> => {
  return http.post('/v1/chat/adventure/start', {
    user_id: userId,
    adventure_folder: adventureFolder,
  })
}

/**
 * 手动检测是否有新冒险可解锁
 */
export const checkUnlocks = async (userId: number): Promise<UnlockedAdventure[]> => {
  const response = await http.post('/v1/chat/adventure/check_unlocks', {
    user_id: userId,
  })
  return response.data || []
}

/**
 * 完成冒险（由剧本引擎结束时调用）
 */
export const completeAdventure = async (userId: number, adventureFolder: string): Promise<any> => {
  return http.post('/v1/chat/adventure/complete', {
    user_id: userId,
    adventure_folder: adventureFolder,
  })
}

/**
 * 重置冒险进度以供重玩
 */
export const resetAdventure = async (userId: number, adventureFolder: string): Promise<any> => {
  return http.post('/v1/chat/adventure/reset', {
    user_id: userId,
    adventure_folder: adventureFolder,
  })
}
