import { API_URLS } from "../consts.ts";
import { CharacterCard } from "../types/CharacterCard.ts";
import { ModelInfo } from "../types/ModelInfo.ts";
export interface Globals {
    model: ModelInfo;
}
export interface Settings {
    current_card: CharacterCard; //当前正在使用的角色卡
    characterCards: CharacterCard[]; //所有可用的角色卡
    // // API 与 模型 设置 END
    // // 对话功能设定 BEGIN // 配置RAG（检索增强生成）系统，让AI能“记忆”历史对话
    // USE_RAG: boolean; // 是否启用RAG系统 [type:bool]
    // USE_TIME_SENSE: boolean; // 是否启用时间感知 [type:bool]
    // // 对话功能设定 END
    // // 基础设置 END
    // // 开发者设置 BEGIN
    // // RAG系统设定 BEGIN // 配置RAG（检索增强生成）系统，让AI能“记忆”历史对话
    // RAG_RETRIEVAL_COUNT: number; // 每次回答时检索的相关历史对话数量
    // RAG_WINDOW_COUNT: number; // 取当前的最新N条消息作为短期记忆，之后则是RAG消息，然后是过去的记忆。
    // RAG_HISTORY_PATH: string; // RAG历史记录存储路径
    // RAG_CHROMA_DB_PATH: string; // ChromaDB向量数据库的存储路径
    // RAG_CANDIDATE_MULTIPLIER: number; // RAG候选乘数，用于计算实际检索的文档数量
    // RAG_CONTEXT_M_BEFORE: number; // RAG检索时考虑当前消息之前的上下文数量
    // RAG_CONTEXT_N_AFTER: number; // RAG检索时考虑当前消息之后的上下文数量
    // RAG_PROMPT_PREFIX: string; // RAG前缀提示，支持多行
    // RAG_PROMPT_SUFFIX: string; // RAG后缀提示，支持多行
    // // RAG系统设定 END
    // // 存储与日志 BEGIN // 配置日志和其他文件的存储位置
    // BACKEND_LOG_DIR: string; // 后端服务日志目录
    // APP_LOG_DIR: string; // 应用行为日志目录
    // ENABLE_FILE_LOGGING: boolean; // 是否将日志记录到文件
    // LOG_FILE_DIRECTORY: string; // 日志文件的存储目录
    // // 存储与日志 END
    // // Debug信息 BEGIN // 用于开发和调试的设置
    // LOG_LEVEL: string; // 日志设置：默认为INFO，设置为DEBUG时启用开发者模式，输出更详尽的日志
    // PRINT_CONTEXT: boolean; // 更改True/False，决定是否把本次发送给llm的全部上下文信息截取后打印到终端
    // // Debug信息 END
    // // 服务端口配置 BEGIN // 配置各个服务的网络监听地址和端口
    // BACKEND_BIND_ADDR: string; // 后端监听地址
    // BACKEND_PORT: number; // 后端监听端口
    // FRONTEND_BIND_ADDR: string; // 前端监听地址
    // FRONTEND_PORT: number; // 前端监听端口
    // EMOTION_BIND_ADDR: string; // 情感分析服务监听地址
    // EMOTION_PORT: number; // 情感分析服务监听端口
    // // 服务端口配置 END
    // // VITS语音与模型 BEGIN // 配置语音合成及其他模型路径
    // SIMPLE_VITS_API_URL: string; // SIMPLE_VITS_API的语音合成API地址
    // STYLE_VITS_API_URL: string; // Style-bert-vits2的语音合成API地址
    // EMOTION_MODEL_PATH: string; // 情感分析模型路径
    // // VITS语音与模型 END
    // // 实验性功能 BEGIN // 配置实验性功能
    // ENABLE_EMOTION_CLASSIFIER: boolean; // 启用/禁用情绪分类器（警告：同时关闭RAG功能后可大幅减少冷启动时间，但表情显示可能不正常）
    // ENABLE_DIRECT_EMOTION_CLASSIFIER: boolean; // 是否在原有情绪可用时直接使用原标签
    // ENABLE_TRANSLAT: boolean; // 是否启用日语翻译功能，而不依赖于LLM的日语（需要新版人物，默认钦灵已适配）
    // TRANSLATE_STREAM: boolean; // 是否启用翻译流式处理（建议开启）
    // OPEN_FRONTEND_AP: boolean; // 是否在启动后端时自动打开前端应用
    // USE_STREAM: boolean; // 是否使用LLM流式生成
    // // 实验性功能 END
    // // 开发者设置 END
}

export async function initSettings(): Promise<Settings> {
    return <Settings>{};
}

export async function initGlobals(): Promise<Globals> {
    return <Globals>{};
}
