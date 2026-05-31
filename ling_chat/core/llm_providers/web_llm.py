from typing import AsyncGenerator, Dict, List

import httpx
from openai import AsyncOpenAI, OpenAI

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class WebLLMProvider(BaseLLMProvider):
    def __init__(self, model_type: str, api_key: str, base_url: str, proxy: str = ""):
        super().__init__()
        self.api_key = api_key
        self.base_url = base_url
        self.model_type = model_type

        # 从LLMConfig读取模型参数
        main_cfg = llm_config.get_main_config()
        self.temperature = main_cfg.get("temperature", 1.3)
        self.top_p = main_cfg.get("top_p", 0.9)
        self.thinking = str(main_cfg.get("enable_thinking", "none")).lower()
        self.max_tokens = int(main_cfg.get("max_tokens", 8192))

        if (not api_key) or api_key == "sk-114514":
            logger.warning("通用网络大模型未初始化：CHAT_API_KEY 为空或为占位值。")
            self.client = None
            return

        if not (
            isinstance(base_url, str)
            and (base_url.startswith("http://") or base_url.startswith("https://"))
        ):
            logger.warning(
                "通用网络大模型未初始化：CHAT_BASE_URL 缺少 http:// 或 https:// 协议头。"
            )
            self.client = None
            return

        self._timeout = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
        http_client = httpx.Client(proxy=proxy, timeout=self._timeout) if proxy else None
        async_http_client = httpx.AsyncClient(proxy=proxy, timeout=self._timeout) if proxy else None
        self.client = OpenAI(api_key=api_key, base_url=base_url, http_client=http_client, timeout=self._timeout)
        self.async_client = AsyncOpenAI(
            api_key=api_key, base_url=base_url, http_client=async_http_client, timeout=self._timeout
        )
        logger.info("通用网络大模型初始化完毕！")

    def _add_thinking_extra_body(self, create_kwargs: dict) -> None:
        """根据 thinking 配置添加 extra_body 参数"""
        if self.thinking in ("true", "false"):
            thinking_enabled = self.thinking == "true"
            create_kwargs["extra_body"] = {
                "thinking": {"type": "enabled" if thinking_enabled else "disabled"},
                "enable_thinking": thinking_enabled,
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
                "max_tokens": self.max_tokens,
                "stream": False,
            }
            self._add_thinking_extra_body(create_kwargs)

            response = self.client.chat.completions.create(**create_kwargs)
            return response.choices[0].message.content

        except Exception as e:
            logger.error(f"通用网络大模型请求失败: {str(e)}")
            raise

    async def generate_stream_response(
        self, messages: List[Dict]
    ) -> AsyncGenerator[str, None]:
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
                "max_tokens": self.max_tokens,
                "stream": True,
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
