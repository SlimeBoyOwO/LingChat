export const API_URLS = {
    WEBSOCKET: "ws://localhost:8765/ws",
    USER_INFO: "/api/user/info",
    USER_DATA: "/api/user/data",
    DEFAULTS: "/api/user/settings/defaults",
    SETTINGS: "/api/user/settings/settings",
    SETTINGS_STRUCTURE: "/api/user/settings/structure",
    CARD: {
        CHARACTER: {
            COVER: "/api/card/character/single/cover",
            EXTEND: "/api/card/character/single/extend",
            SINGLE: "/api/card/character/single/full",
            SEARCH: "/api/card/character/search"
        }
    },
    IMAGE: {
        BACKGROUND: "/api/image/background"
    },
    AUDIO: {
        BACKGROUND: "/api/audio/background",
        EFFECT: "/api/audio/effect",
        AVATAR: "/api/audio/avatar"
    }
} as const;

export const CONFIG = {
    WEBSOCKET: {
        MAX_RECONNECTS: 5,
        HEARTBEAT_INTERVAL: 30000,
        RECONNECT_DELAY_BASE: 1000,
        MAX_RECONNECT_DELAY: 30000
    },
    DEFAULT_BACKGROUND: "/src/assets/images/background.png"
} as const;

export const LIMITS = {
    MAX_SAVE_COUNT: 10,
    MAX_AVATAR_COUNT: 6
} as const;

export const PAGES = {
    MAIN: {
        MENU: "MenuView",
        SETTINGS: "SettingsView",
        CHAT: "ChatView"
    },
    SETTINGS: {
        SAVE: "save",
        HISTORY: "history"
    }
} as const;

export const CHAT_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio"] as const;

export const TRANSLATE_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio", "qwen-translate"] as const;
