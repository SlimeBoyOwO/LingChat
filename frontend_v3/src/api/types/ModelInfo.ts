interface OllamaModelInfo {
    base_url: string;
    model: string;
}

interface LMStudioModelInfo {
    model_type: string;
    base_url: string;
    api_key?: string;
}

interface GeminiModelInfo {
    model_type: string;
    base_url: string;
}

//聊天生成模型
interface ChatModelInfo {
    llm_provider: string;
    api_key: string;
    base_url: string;
    model_type: string;
    model_info: OllamaModelInfo | LMStudioModelInfo | GeminiModelInfo | null;
}
// 视觉模型
interface VisualModelInfo {
    api_key: string;
    base_url: string;
    model: string;
}
// 翻译模型
interface TranslateModelInfo {
    llm_provider: string;
    api_key: string;
}

export interface ModelInfo {
    chat: ChatModelInfo;
    visual?: VisualModelInfo;
    translate?: TranslateModelInfo;
}
