export interface OllamaModelInfo {
    base_url: string;
    model: string;
}

export interface LMStudioModelInfo {
    model_type: string;
    base_url: string;
    api_key?: string;
}

export interface GeminiModelInfo {
    model_type: string;
    base_url: string;
}

export interface ChatModelInfo {
    llm_provider: string;
    api_key: string; // DeepSeek 或其他聊天模型的 API Key
    base_url: string; // API的访问地址
    model_type: string; // 使用的模型类型
    model_info: OllamaModelInfo | LMStudioModelInfo | GeminiModelInfo | null;
}

export interface VisualModelInfo {
    api_key: string; // 图像识别模型的 API Key
    base_url: string; // 视觉模型的API访问地址
    model: string; // 视觉模型的模型类型
}

export interface TranslateModelInfo {
    llm_provider: string; // 翻译模型提供者（同对话模型+一个 qwen-translate ）
    api_key: string; // 翻译模型的 API Key
}

export interface ModelInfo {
    chat: ChatModelInfo;
    visual?: VisualModelInfo;
    translate?: TranslateModelInfo;
}
