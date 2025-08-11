export const API_URLS = {
    USER_INFO: "/api/user_info",
    CHARACTER_CARD: "/api/character_card"
} as const;
export const CHAT_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio"] as const;

export const TRANSLATE_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio", "qwen-translate"] as const;
