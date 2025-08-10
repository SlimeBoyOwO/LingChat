export const API_URLS = {
    user_info: "/api/user/info",
    character_card: "/api/character/card"
};
export const CHAT_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio"] as const;

export const TRANSLATE_LLM_PROVIDERS = ["webllm", "gemini", "ollama", "lmstudio", "qwen-translate"] as const;
