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
}

// 2. 定义完整的初始化数据接口 (对应 Rust WebInitData)
export interface WebInitData {
  character_settings: CharacterSettings
  current_interact_role_id: number | null
  onstage_roles_ids: number[]
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

/**
 * 获取 TTS 生成的语音文件，返回 base64 data URL
 */
export const getVoiceAudio = async (fileName: string): Promise<string> => {
  return await invoke<string>('get_voice_audio', { fileName })
}
