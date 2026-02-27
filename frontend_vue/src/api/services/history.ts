import http from '../http'

export interface GameLine {
  id?: number
  content: string
  original_emotion?: string | null
  predicted_emotion?: string | null
  tts_content?: string | null
  action_content?: string | null
  audio_file?: string | null
  attribute: string // LineAttribute 枚举值，如 "NORMAL", "SYSTEM" 等
  sender_role_id?: number | null
  display_name?: string | null
  perceived_role_ids: number[]
}

export const getDialogueHistory = async (userId: string): Promise<GameLine[]> => {
  try {
    const data = await http.get('/v1/chat/history/get_chat_lines', {
      params: { user_id: userId },
    })
    return data
  } catch (error: any) {
    console.error('获取游戏历史信息错误:', error.message)
    throw error // 直接抛出拦截器处理过的错误
  }
}
