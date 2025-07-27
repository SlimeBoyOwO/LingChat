import type { ApiConfig } from "../../types";

const API_BASE = "/api/v1";

export const API_CONFIG: ApiConfig = {
  // 头像相关
  AVATAR: {
    BASE: `${API_BASE}/chat/character/get_avatar`,
    DEFAULT: "default.png",
  },

  VOICE: {
    BASE: `${API_BASE}/chat/sound/get_voice`,
  },

  // 认证相关
  AUTH: `${API_BASE}/auth`,
};

// 应用配置
export const APP_CONFIG = {
  // WebSocket配置
  WEBSOCKET: {
    MAX_RECONNECTS: 5,
    HEARTBEAT_INTERVAL: 30000,
    RECONNECT_DELAY_BASE: 1000,
    MAX_RECONNECT_DELAY: 30000,
  },

  // UI配置
  UI: {
    LOADER_MIN_DURATION: 1500,
    ANIMATION_DURATION: 300,
  },

  // 星空背景配置
  STAR_FIELD: {
    STAR_COUNT: 100,
    SPEED: 0.5,
    COLOR: "#ffffff",
    SIZE: 1,
  },
} as const;

export type AppConfig = typeof APP_CONFIG;
