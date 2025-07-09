from typing import Dict, List
from .provider_factory import LLMProviderFactory
from .base import BaseLLMProvider
from core.logger import logger
import os

class LLMManager:
    def __init__(self):
        """
        初始化LLM管理器
        
        :param provider_config: 可选，提供者配置字典。如果为None，则从环境变量加载
        """
        self.llm_provider_type = os.environ.get("LLM_PROVIDER", "webllm").lower()
        self.provider = self._initialize_provider()
    
    def _initialize_provider(self) -> 'BaseLLMProvider':
        """
        初始化大模型提供者
        
        :param provider_config: 提供者配置字典
        :return: 初始化的大模型提供者实例
        """
        # 确保provider_type存在
        provider_type = self.llm_provider_type.lower()
        
        logger.info(f"初始化大模型 {provider_type} 提供商中...")
        return LLMProviderFactory.create_provider(provider_type)
    
    def process_message(self, messages: List[Dict]):
        return self.provider.generate_response(messages)