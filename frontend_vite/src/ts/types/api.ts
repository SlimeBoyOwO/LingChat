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
