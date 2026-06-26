import http from '../http'

// 1. 定义角色配置接口 (原先摊平的字段现在归属到这里)
export interface CharacterSettings {
  ai_name: string
  ai_subtitle: string
  user_name: string
  user_subtitle: string
  character_id: number
  thinking_message: string
  scale: number
  offset_x: number
  offset_y: number
  bubble_top: number
  bubble_left: number
  clothes: Record<string, any>
  clothes_name: string
  body_part: Record<string, any>
  character_folder: string
}

// 2. 定义完整的初始化数据接口 (对应后端的 WebInitData)
export interface WebInitData {
  character_settings: CharacterSettings
  current_interact_role_id: number | null
  onstage_roles_ids: number[]
  background: string
  background_effect: string
  background_music: string
}

export interface VoiceCacheStats {
  cache_dir: string
  total_bytes: number
  total_size_mb: number
  file_count: number
  saved_voice_count: number
  saved_file_count: number
  missing_file_count: number
}

export interface VoiceCacheClearResult {
  deleted_files: number
  deleted_bytes: number
  deleted_size_mb: number
  cleared_db_refs: number
  cleared_runtime_refs: number
  failed_files: string[]
  stats: VoiceCacheStats
}

/**
 * 获取游戏初始化信息
 * @param client_id 客户端唯一标识
 * @param userId 用户ID
 */
export const getGameInfo = async (client_id: string, userId: string): Promise<WebInitData> => {
  try {
    const data = await http.get('/v1/chat/info/init', {
      params: { client_id: client_id, user_id: userId },
    })

    console.log('成功获取初始化数据:', data)
    return data
  } catch (error: any) {
    console.error('获取初始化信息错误:', error.message)
    throw error
  }
}

export const reactivateTTS = async (): Promise<void> => {
  try {
    const data = await http.get('/v1/chat/info/reactivate', {})

    console.log('成功重启TTS服务')
    return data
  } catch (error: any) {
    console.error('TTS服务重启错误:', error.message)
    throw error
  }
}

export const getVoiceCacheStats = async (): Promise<VoiceCacheStats> => {
  return http.get('/v1/chat/cache/voice', { silent: true })
}

export const clearVoiceCache = async (): Promise<VoiceCacheClearResult> => {
  return http.delete('/v1/chat/cache/voice')
}
