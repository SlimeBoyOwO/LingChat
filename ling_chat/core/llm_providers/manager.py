from typing import Dict, List
from ling_chat.core.llm_providers.provider_factory import LLMProviderFactory
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger
import asyncio
import os

class LLMManager:
    def __init__(self, llm_job = None):
        """
        初始化LLM管理器
        
        :param provider_config: 可选，提供者配置字典。如果为None，则从环境变量加载
        """
        if not llm_job or llm_job == "main":
            self.llm_provider_type = os.environ.get("LLM_PROVIDER", "webllm")
            self.model_type = os.environ.get("MODEL_TYPE", "deepseek-chat")
            self.api_key = os.environ.get("CHAT_API_KEY", "")
            self.api_url = os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com/v1")
            # 确保provider_type存在
            provider_type = self.llm_provider_type.lower()
            logger.info(f"初始化LLM {provider_type} 提供商中...")
        elif llm_job == "translator":
            self.llm_provider_type = os.environ.get("TRANSLATE_LLM_PROVIDER", "qwen-translate")
            self.model_type = os.environ.get("TRANSLATE_MODEL", "")
            self.api_key = os.environ.get("TRANSLATE_API_KEY", "")
            self.api_url = os.environ.get("TRANSLATE_BASE_URL", "")
            # 确保provider_type存在
            provider_type = self.llm_provider_type.lower()
            logger.info(f"初始化翻译模型 {provider_type} 提供商中...")
        
        self.provider = self._initialize_provider()
    
    def _initialize_provider(self) -> 'BaseLLMProvider':
        """
        初始化大模型提供者
        
        :param provider_config: 提供者配置字典
        :return: 初始化的大模型提供者实例
        """
        # 确保provider_type存在
        provider_type = self.llm_provider_type.lower()
        if provider_type == "webllm":
            return LLMProviderFactory.create_provider(provider_type, 
                                                      self.model_type, self.api_key, self.api_url)
        else:
            return LLMProviderFactory.create_provider(provider_type)
    
    def process_message(self, messages: List[Dict]):
        return self.provider.generate_response(messages)

    async def process_message_stream(self, messages: List[Dict]):
        async for chunk in self.provider.generate_stream_response(messages):
            yield chunk
            await asyncio.sleep(0.05)  # 关键：在每个chunk后让出控制权
