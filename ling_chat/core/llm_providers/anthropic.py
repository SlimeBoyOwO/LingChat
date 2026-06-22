"""Anthropic Claude LLM Provider

支持 Anthropic Claude API 的适配器，包括：
- 同步和异步客户端
- 流式和非流式响应
- 自定义 base_url（支持代理和第三方兼容 API）
- system prompt 支持
- temperature、top_p、top_k 等参数配置

文档：https://docs.anthropic.com/
"""

from typing import AsyncGenerator, Dict, List, Optional

import httpx
from anthropic import Anthropic, AsyncAnthropic

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.llm_providers._http import build_httpx_client
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class AnthropicProvider(BaseLLMProvider):
    """Anthropic Claude API Provider"""

    def __init__(self, model_type: str = "", api_key: str = "", base_url: str = ""):
        super().__init__()
        main_cfg = llm_config.get_main_config()

        self.api_key = api_key or main_cfg.get("api_key", "")
        self.model_type = model_type or main_cfg.get("model", "claude-sonnet-4-5-20250929")
        self.base_url = base_url or main_cfg.get("base_url", "https://api.anthropic.com")
        self.temperature = main_cfg.get("temperature", 1.0)
        self.top_p = main_cfg.get("top_p", 0.9)
        self.top_k = main_cfg.get("top_k", 40)
        self.max_tokens = int(main_cfg.get("max_tokens", 8192))

        # API key 验证
        if not self.api_key:
            logger.warning("Anthropic 未初始化：API_KEY 为空")
            self.client = None
            self.async_client = None
            return

        self._timeout = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)

        # 构建同步客户端
        http_client = build_httpx_client(timeout=self._timeout, base_url=self.base_url)
        self.client = Anthropic(
            api_key=self.api_key,
            base_url=self.base_url,
            http_client=http_client,
            timeout=self._timeout,
        )

        # 构建异步客户端
        async_http_client = build_httpx_client(
            async_client=True, timeout=self._timeout, base_url=self.base_url
        )
        self.async_client = AsyncAnthropic(
            api_key=self.api_key,
            base_url=self.base_url,
            http_client=async_http_client,
            timeout=self._timeout,
        )

        logger.info(f"Anthropic Claude 初始化完毕，模型: {self.model_type}")

    def initialize_client(self):
        """客户端已在 __init__ 中初始化"""
        pass

    def _extract_system_and_messages(
        self, messages: List[Dict]
    ) -> tuple[Optional[str], List[Dict]]:
        """
        从 OpenAI 格式消息中提取 system prompt 并转换角色名称

        Anthropic API 将 system prompt 作为独立参数传递，而非消息列表中的一条

        Args:
            messages: OpenAI 格式的消息列表

        Returns:
            (system_prompt, converted_messages) 元组
        """
        system_prompt = None
        converted_messages = []

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            # system 消息提取为独立参数
            if role == "system":
                system_prompt = str(content)
                continue

            # 转换角色名称：human -> user, assistant -> assistant（不变）
            if role == "human":
                role = "user"

            # Anthropic 要求 content 为字符串或内容块列表
            converted_messages.append({
                "role": role,
                "content": str(content),
            })

        return system_prompt, converted_messages

    def generate_response(self, messages: List[Dict]) -> str:
        """生成非流式响应"""
        if self.client is None:
            error_message = "Anthropic 未初始化，请检查配置"
            logger.error(error_message)
            return error_message

        try:
            logger.debug(f"正在向 Anthropic 发送请求: {self.model_type}")

            system_prompt, converted_messages = self._extract_system_and_messages(messages)

            # 构建请求参数
            create_kwargs = {
                "model": self.model_type,
                "max_tokens": self.max_tokens,
                "messages": converted_messages,
            }

            # 添加 system prompt（如果有）
            if system_prompt:
                create_kwargs["system"] = system_prompt

            # 添加可选参数（Anthropic 不支持 temperature=0 时同时设置 top_p/top_k）
            if self.temperature > 0:
                create_kwargs["temperature"] = self.temperature
                if self.top_p and self.top_p < 1.0:
                    create_kwargs["top_p"] = self.top_p
                if self.top_k:
                    create_kwargs["top_k"] = self.top_k

            response = self.client.messages.create(**create_kwargs)

            # Anthropic 返回 content 列表，提取文本内容
            text_content = ""
            for block in response.content:
                if hasattr(block, "text"):
                    text_content += block.text

            return text_content

        except Exception as e:
            logger.error(f"Anthropic 请求失败: {str(e)}")
            raise

    async def generate_stream_response(
        self, messages: List[Dict]
    ) -> AsyncGenerator[str, None]:
        """生成流式响应"""
        if self.async_client is None:
            error_message = "Anthropic 未初始化，请检查配置"
            logger.error(error_message)
            raise ConnectionError(error_message)

        try:
            logger.debug(f"正在向 Anthropic 发送流式请求: {self.model_type}")

            system_prompt, converted_messages = self._extract_system_and_messages(messages)

            # 构建请求参数
            create_kwargs = {
                "model": self.model_type,
                "max_tokens": self.max_tokens,
                "messages": converted_messages,
            }

            # 添加 system prompt
            if system_prompt:
                create_kwargs["system"] = system_prompt

            # 添加可选参数
            if self.temperature > 0:
                create_kwargs["temperature"] = self.temperature
                if self.top_p and self.top_p < 1.0:
                    create_kwargs["top_p"] = self.top_p
                if self.top_k:
                    create_kwargs["top_k"] = self.top_k

            # 使用 messages.stream() 进行流式调用
            async with self.async_client.messages.stream(**create_kwargs) as stream:
                async for text in stream.text_stream:
                    yield text

        except Exception as e:
            logger.error(f"Anthropic 流式请求失败: {str(e)}")
            import traceback
            traceback.print_exc()
            raise