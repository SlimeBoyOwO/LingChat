from typing import AsyncGenerator, Dict, List

import httpx
from openai import AsyncOpenAI, OpenAI
import os

from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class WebLLMProvider(BaseLLMProvider):
    def __init__(self, model_type: str, api_key: str, base_url: str):
        super().__init__()
        self.api_key = api_key
        self.base_url = base_url
        self.model_type = model_type
        self.temperature = float(os.environ.get("TEMPERATURE", 1.3))
        self.top_p = float(os.environ.get("TOP_P", 0.9))
        self.thinking = os.environ.get("ENABLE_THINKING", "None").lower()

        if (not api_key) or api_key == "sk-114514":
            logger.warning("通用网络大模型未初始化：CHAT_API_KEY 为空或为占位值。")
            self.client = None
            return

        if not (isinstance(base_url, str) and (base_url.startswith("http://") or base_url.startswith("https://"))):
            logger.warning("通用网络大模型未初始化：CHAT_BASE_URL 缺少 http:// 或 https:// 协议头。")
            self.client = None
            return

        self._timeout = httpx.Timeout(15.0)
        self.client = OpenAI(api_key=api_key, base_url=base_url, timeout=self._timeout)
        self.async_client = AsyncOpenAI(api_key=api_key, base_url=base_url, timeout=self._timeout)
        logger.info("通用网络大模型初始化完毕！" )

    def _add_thinking_extra_body(self, create_kwargs: dict) -> None:
        """根据 thinking 配置添加 extra_body 参数"""
        if self.thinking in ("true", "false"):
            thinking_enabled = self.thinking == "true"
            create_kwargs["extra_body"] = {
                "thinking": {"type": "enabled" if thinking_enabled else "disabled"},
                "enable_thinking": thinking_enabled
            }

    def initialize_client(self):
        return super().initialize_client()

    def generate_response(self, messages: List[Dict]) -> str:
        """生成模型响应"""
        if self.client is None:
            error_message = "通用网络大模型未初始化，请检查配置"
            logger.error(error_message)
            return error_message

        try:
            logger.debug(f"正在对通用网络大模型发送请求: {self.model_type}")
            # 构建基础参数
            create_kwargs = {
                "model": self.model_type,
                "messages": messages,
                "temperature": self.temperature,
                "top_p": self.top_p,
                "stream": False
            }
            self._add_thinking_extra_body(create_kwargs)

            response = self.client.chat.completions.create(**create_kwargs)
            return response.choices[0].message.content

        except Exception as e:
            logger.error(f"通用网络大模型请求失败: {str(e)}")
            raise

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """
        生成流式响应
        """
        if self.async_client is None:
            error_message = "通用网络大模型未初始化，请检查配置"
            logger.error(error_message)
            yield error_message
            return

        try:
            logger.debug(f"正在对通用网络大模型发送流式请求: {self.model_type}")

            # 构建基础参数
            create_kwargs = {
                "model": self.model_type,
                "messages": messages,
                "temperature": self.temperature,
                "top_p": self.top_p,
                "stream": True
            }
            self._add_thinking_extra_body(create_kwargs)

            stream = await self.async_client.chat.completions.create(**create_kwargs)

            async for chunk in stream:
                if chunk.choices[0].delta.content is not None:
                    yield chunk.choices[0].delta.content

        except Exception as e:
            logger.error(f"通用网络大模型{self.model_type}流式请求失败: {str(e)}")
            import traceback
            traceback.print_exc()
            raise
