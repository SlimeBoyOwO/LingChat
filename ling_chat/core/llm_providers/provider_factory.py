from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.llm_providers.gemini import GeminiProvider
from ling_chat.core.llm_providers.lmstudio import LMStudioProvider
from ling_chat.core.llm_providers.ollama import OllamaProvider
from ling_chat.core.llm_providers.qwen_translate import QwenTranslateProvider
from ling_chat.core.llm_providers.web_llm import WebLLMProvider
from ling_chat.core.logger import logger


class LLMProviderFactory:
    @staticmethod
    def create_provider(provider_type: str,
                        model_type: str="", api_key: str="", base_url: str="") -> BaseLLMProvider:
        """
        创建指定类型的大模型提供者
        
        :param provider_type: 提供者类型 (webllm, ollama, lmstudio, gemini, qwen)
        :param config: 配置字典
        :return: 大模型提供者实例
        """
        provider_type = provider_type.lower()

        try:
            if provider_type == "webllm":
                logger.info("创建通用联网大模型服务提供商")
                return WebLLMProvider(model_type, api_key, base_url)
            elif provider_type == "ollama":
                logger.info("创建OLLAMA服务提供商")
                return OllamaProvider()
            elif provider_type == "lmstudio":
                logger.info("创建LM STUDIO服务提供商")
                return LMStudioProvider()
            elif provider_type == "gemini":
                logger.info("创建Gemini服务提供商")
                return GeminiProvider()
            elif provider_type == "qwen-translate":
                logger.info("创建Qwen翻译服务提供商")
                return QwenTranslateProvider()
            else:
                raise ValueError(f"暂未支持的提供商: {provider_type}")
        except Exception as e:
            logger.error(f"创建服务提供商 {provider_type} 失败: {str(e)}")
            raise
