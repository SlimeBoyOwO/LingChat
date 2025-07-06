from .web_llm import WebLLMProvider
from .ollama import OllamaProvider
from .lmstudio import LMStudioProvider
from .gemini import GeminiProvider
from .base import BaseLLMProvider
from typing import Dict, List
from core.logger import logger

class LLMProviderFactory:
    @staticmethod
    def create_provider(provider_type: str) -> BaseLLMProvider:
        """
        创建指定类型的大模型提供者
        
        :param provider_type: 提供者类型 (deepseek, ollama, lmstudio)
        :param config: 配置字典
        :return: 大模型提供者实例
        """
        provider_type = provider_type.lower()
        
        try:
            if provider_type == "webllm":
                logger.info("创建通用联网大模型提服务提供商")
                return WebLLMProvider()
            elif provider_type == "ollama":
                logger.info("创建OLLAMA服务提供商")
                return OllamaProvider()
            elif provider_type == "lmstudio":
                logger.info("创建LM STUDIO服务提供商")
                return LMStudioProvider()
            elif provider_type == "gemini":
                logger.info("创建Gemini服务提供商")
                return GeminiProvider()
            else:
                raise ValueError(f"Unsupported provider type: {provider_type}")
        except Exception as e:
            logger.error(f"创建LLM提供商 {provider_type} 失败: {str(e)}")
            raise