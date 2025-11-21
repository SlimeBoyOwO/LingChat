// API相关类型定义

export interface ApiConfig {
  AVATAR: {
    BASE: string;
    DEFAULT: string;
  };
  VOICE: {
    BASE: string;
  };
  AUTH: string;
}

export interface ApiResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  message?: string;
}

export interface AuthResponse {
  token: string;
  user: {
    id: string;
    username: string;
    avatar?: string;
  };
}

export interface AvatarInfo {
  url: string;
  name: string;
  size?: number;
}

export interface BackgroundImageInfo {
  title: string;
  url: string;
  time: string;
}

export interface Character {
  character_id: string;
  title: string;
  info: string;
  avatar_path: string;
}

export interface CharacterSelectParams {
  user_id: string;
  character_id: string;
}

export interface SaveInfo {
  id: string;
  title: string;
  updated_at: string;
}

export interface SaveListParams {
  user_id: string;
  page: number;
  page_size: number;
}

export interface SaveModifyParams {
  user_id: string;
  conversation_id: string;
}

export interface SaveCreateParams {
  user_id: string;
  title: string;
}

export interface MusicTrack {
  name: string;
  url: string;
}
