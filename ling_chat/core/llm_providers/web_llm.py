from typing import AsyncGenerator, Dict, List

import httpx
from openai import AsyncOpenAI, OpenAI

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.llm_providers._http import build_httpx_client
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.llm_providers.tool_types import (
    LLMResponse,
    ToolCall,
    ToolDefinition,
)
from ling_chat.core.logger import logger


class WebLLMProvider(BaseLLMProvider):
    def __init__(self, model_type: str, api_key: str, base_url: str):
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
            logger.warning(
                "通用网络大模型未初始化：API_KEY 为空或为占位值，请到 LLM 配置页面设置。"
            )
            self.client = None
            self.async_client = None
            return

        if not (
            isinstance(base_url, str)
            and (base_url.startswith("http://") or base_url.startswith("https://"))
        ):
            logger.warning(
                "通用网络大模型未初始化：BASE_URL 缺少 http:// 或 https:// 协议头，请到 LLM 配置页面检查。"
            )
            self.client = None
            self.async_client = None
            return

        self._timeout = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
        http_client = build_httpx_client(timeout=self._timeout, base_url=base_url)
        async_http_client = build_httpx_client(
            async_client=True, timeout=self._timeout, base_url=base_url
        )
        self.client = OpenAI(
            api_key=api_key,
            base_url=base_url,
            http_client=http_client,
            timeout=self._timeout,
        )
        self.async_client = AsyncOpenAI(
            api_key=api_key,
            base_url=base_url,
            http_client=async_http_client,
            timeout=self._timeout,
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
            raise ConnectionError(error_message)

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

    def generate_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> LLMResponse:
        """生成带工具调用的响应 (OpenAI 原生支持)"""
        if self.client is None:
            error_message = "通用网络大模型未初始化，请检查配置"
            logger.error(error_message)
            return LLMResponse(content=error_message)

        try:
            logger.debug(f"正在对通用网络大模型发送工具调用请求: {self.model_type}")

            # 转换工具定义为 OpenAI 格式
            tools_openai = [tool.model_dump() for tool in tools]

            create_kwargs = {
                "model": self.model_type,
                "messages": messages,
                "tools": tools_openai,
                "temperature": self.temperature,
                "top_p": self.top_p,
                "max_tokens": self.max_tokens,
                "stream": False,
            }
            self._add_thinking_extra_body(create_kwargs)

            response = self.client.chat.completions.create(**create_kwargs)
            message = response.choices[0].message

            # 解析工具调用
            tool_calls = []
            if hasattr(message, "tool_calls") and message.tool_calls:
                for tc in message.tool_calls:
                    tool_calls.append(
                        ToolCall.from_openai(
                            {
                                "id": tc.id,
                                "type": tc.type,
                                "function": {
                                    "name": tc.function.name,
                                    "arguments": tc.function.arguments,
                                },
                            }
                        )
                    )

            return LLMResponse(content=message.content or "", tool_calls=tool_calls)

        except Exception as e:
            logger.error(f"通用网络大模型工具调用请求失败: {str(e)}")
            raise

    async def generate_stream_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> AsyncGenerator[LLMResponse, None]:
        """生成带工具调用的流式响应"""
        if self.async_client is None:
            error_message = "通用网络大模型未初始化，请检查配置"
            logger.error(error_message)
            raise ConnectionError(error_message)

        try:
            logger.debug(f"正在对通用网络大模型发送工具调用流式请求: {self.model_type}")

            tools_openai = [tool.model_dump() for tool in tools]

            create_kwargs = {
                "model": self.model_type,
                "messages": messages,
                "tools": tools_openai,
                "temperature": self.temperature,
                "top_p": self.top_p,
                "max_tokens": self.max_tokens,
                "stream": True,
            }
            self._add_thinking_extra_body(create_kwargs)

            stream = await self.async_client.chat.completions.create(**create_kwargs)
            accumulated_content = ""
            tool_calls_data = {}

            async for chunk in stream:
                delta = chunk.choices[0].delta

                # 处理文本内容
                if delta.content:
                    accumulated_content += delta.content
                    yield LLMResponse(content=delta.content, is_finished=False)

                # 处理工具调用（流式）
                # OpenAI 流式 tool_calls 使用 index 作为分组键
                # id 只在第一个 chunk 中出现，后续 chunk 通过 index 关联
                if hasattr(delta, "tool_calls") and delta.tool_calls:
                    for tc in delta.tool_calls:
                        # 使用 index 作为分组键（每个工具调用有固定 index）
                        idx = (
                            tc.index
                            if hasattr(tc, "index") and tc.index is not None
                            else 0
                        )
                        if idx not in tool_calls_data:
                            tool_calls_data[idx] = {
                                "id": tc.id or "",
                                "name": "",
                                "arguments": "",
                            }
                        # 首个 chunk 可能携带 id
                        if tc.id and not tool_calls_data[idx]["id"]:
                            tool_calls_data[idx]["id"] = tc.id
                        if tc.function:
                            if tc.function.name:
                                tool_calls_data[idx]["name"] = tc.function.name
                            if tc.function.arguments:
                                tool_calls_data[idx]["arguments"] += (
                                    tc.function.arguments
                                )

            # 构建最终工具调用
            tool_calls = []
            for tc_data in tool_calls_data.values():
                if tc_data["name"]:
                    import json

                    try:
                        args = (
                            json.loads(tc_data["arguments"])
                            if tc_data["arguments"]
                            else {}
                        )
                    except json.JSONDecodeError:
                        args = {}
                    tool_calls.append(
                        ToolCall(id=tc_data["id"], name=tc_data["name"], arguments=args)
                    )

            yield LLMResponse(content="", tool_calls=tool_calls, is_finished=True)

        except Exception as e:
            logger.error(f"通用网络大模型工具调用流式请求失败: {str(e)}")
            import traceback

            traceback.print_exc()
            raise
