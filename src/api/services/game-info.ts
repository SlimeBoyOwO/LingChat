import { invoke } from '@tauri-apps/api/core'
import type { SceneInfo } from './scene'

// 1. 定义角色配置接口 (原先摊平的字段现在归属到这里)
export interface CharacterSettings {
  ai_name: string
  ai_subtitle: string
  user_name: string
  user_subtitle: string
  character_id: number | null
  thinking_message: string
  scale: number
  offset_x: number
  offset_y: number
  scale_p: number
  offset_x_p: number
  offset_y_p: number
  bubble_top: number
  bubble_left: number
  clothes: Record<string, any>
  clothes_name: string
  body_part: Record<string, any>
  character_folder: string
}

/// 前端用台词条目（对应 Rust GameLineInit）
export interface GameLineInit {
  content: string
  attribute: string
  sender_role_id: number | null
  display_name: string | null
  original_emotion: string | null
  predicted_emotion: string | null
  action_content: string | null
  audio_file: string | null
  perceived_role_ids: number[]
  /** 玩家消息序号（1-indexed），仅 sender_role_id == 0 的 user 行有值 */
  user_message_seq: number | null
}

// 2. 定义完整的初始化数据接口 (对应 Rust WebInitData)
export interface WebInitData {
  character_settings: CharacterSettings
  current_interact_role_id: number | null
  onstage_roles_ids: number[]
  /** 在场角色的完整设定（含主角与非主角），用于初始化 gameRoles / presentRoleIds */
  onstage_roles: CharacterSettings[]
  background: string
  background_effect: string
  background_music: string
  current_scene_id: string | null
  current_scene: SceneInfo | null
  lines: GameLineInit[]
  scene_awareness_enabled: boolean
}

/**
 * 获取游戏初始化信息（Tauri invoke）
 */
export const getGameInfo = async (): Promise<WebInitData> => {
  try {
    const data = await invoke<WebInitData>('init_game')
    console.log('获取初始化信息成功:', data)
    return data
  } catch (error: any) {
    console.error('获取初始化信息错误:', typeof error === 'string' ? error : error.message)
    throw error
  }
}

export const reactivateTTS = async (): Promise<void> => {
  try {
    await invoke('reactivate_tts')
    console.log('成功重启TTS服务')
  } catch (error: any) {
    console.error('TTS服务重启错误:', typeof error === 'string' ? error : error.message)
    throw error
  }
}

export const clearTtsCache = async (): Promise<{ success: boolean; message: string; deleted: number }> => {
  try {
    const result = await invoke<{ success: boolean; message: string; deleted: number }>('clear_tts_cache')
    console.log('清理TTS缓存成功:', result)
    return result
  } catch (error: any) {
    console.error('清理TTS缓存错误:', typeof error === 'string' ? error : error.message)
    throw error
  }
}

export const getVoiceCleanupInfo = async (): Promise<{ deleted: number; hasRun: boolean }> => {
  try {
    const result = await invoke<{ deleted: number; hasRun: boolean }>('get_voice_cleanup_info')
    console.log('获取语音自动清理信息成功:', result)
    return result
  } catch (error: any) {
    console.error('获取语音自动清理信息错误:', typeof error === 'string' ? error : error.message)
    return { deleted: 0, hasRun: false }
  }
}

/**
 * 获取 TTS 生成的语音文件，返回 base64 data URL
 */
export const getVoiceAudio = async (fileName: string): Promise<string> => {
  return await invoke<string>('get_voice_audio', { fileName })
}
