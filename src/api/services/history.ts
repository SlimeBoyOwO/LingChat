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

