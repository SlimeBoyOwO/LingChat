export const API_URLS = {
    USER_INFO: "/api/user/info",
    DEFAULTS: "/api/user/settings/defaults",
    SETTINGS: "/api/user/settings/settings",
    CARD: {
        CHARACTER: {
            COVER: "/api/card/character/single/cover",
            EXTEND: "/api/card/character/single/extend",
            SINGLE: "/api/card/character/single/full",
            SEARCH: "/api/card/character/search"
        }
    }
} as const;
export const CHAT_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio"] as const;

export const TRANSLATE_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio", "qwen-translate"] as const;
