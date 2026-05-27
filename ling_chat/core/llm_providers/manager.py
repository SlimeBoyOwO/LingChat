from typing import Dict, List

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.llm_providers.provider_factory import LLMProviderFactory
from ling_chat.core.logger import logger


class LLMManager:
    def __init__(self, llm_job=None):
        """
        初始化LLM管理器

        :param llm_job: 可选，"main"或"translator"，决定使用哪个配置
        """
        if not llm_job or llm_job == "main":
            cfg = llm_config.get_main_config()
        elif llm_job == "translator":
            cfg = llm_config.get_translator_config()

        self.llm_provider_type = cfg.get("provider", "webllm")
        self.model_type = cfg.get("model", "deepseek-chat")
        self.api_key = cfg.get("api_key", "")
        self.base_url = cfg.get("base_url", "https://api.deepseek.com/v1")
        self.proxy = cfg.get("proxy", "")
        provider_type = self.llm_provider_type.lower()
        logger.info(f"初始化LLM {provider_type} 提供商中...")

        self.provider = self._initialize_provider()

    def _initialize_provider(self) -> "BaseLLMProvider":
        """
        初始化大模型提供者

        :param provider_config: 提供者配置字典
        :return: 初始化的大模型提供者实例
        """
        # 确保provider_type存在
        provider_type = self.llm_provider_type.lower()
        return LLMProviderFactory.create_provider(
            provider_type, self.model_type, self.api_key, self.base_url, self.proxy
        )

    def process_message(self, messages: List[Dict]):
        return self.provider.generate_response(messages)

    async def process_message_stream(self, messages: List[Dict]):
        async for chunk in self.provider.generate_stream_response(messages):
            yield chunk
