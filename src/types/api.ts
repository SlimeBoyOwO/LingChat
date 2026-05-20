// API相关类型定义

export interface ApiConfig {
  VOICE: {
    BASE: string
  }
  AUTH: string
}

export interface ApiResponse<T = any> {
  success: boolean
  data?: T
  error?: string
  message?: string
}

export interface AuthResponse {
  token: string
  user: {
    id: string
    username: string
    avatar?: string
  }
}

export interface AvatarInfo {
  url: string
  name: string
  size?: number
}

export interface BackgroundImageInfo {
  title: string
  url: string
  time: string
}

export interface Clothes {
  title: string
  avatar: string
}

export interface Character {
  character_id: string
  title: string
  name: string
  sub_name: string
  info: string
  avatar_path: string
  clothes: Array<Clothes>
  resource_folder?: string
  adventure_count?: number
  total_adventures?: number
}

export interface CharacterSelectParams {
  user_id: string
  character_id: string
}

export interface SaveInfo {
  id: number
  title: string
  update_date: string
  create_date: string
}

export interface MusicTrack {
  name: string
  url: string
  time: string
}
