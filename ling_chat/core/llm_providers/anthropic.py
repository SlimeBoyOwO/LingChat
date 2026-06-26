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
from ling_chat.core.llm_providers.tool_types import (
    LLMResponse,
    ToolCall,
    ToolConverter,
    ToolDefinition,
)
from ling_chat.core.logger import logger


class AnthropicProvider(BaseLLMProvider):
    """Anthropic Claude API Provider"""

    def __init__(self, model_type: str = "", api_key: str = "", base_url: str = ""):
        super().__init__()
        main_cfg = llm_config.get_main_config()

        self.api_key = api_key or main_cfg.get("api_key", "")
        self.model_type = model_type or main_cfg.get(
            "model", "claude-sonnet-4-5-20250929"
        )
        self.base_url = base_url or main_cfg.get(
            "base_url", "https://api.anthropic.com"
        )
        self.temperature = main_cfg.get("temperature", 1.0)
        self.top_p = main_cfg.get("top_p", 0.9)
        self.top_k = main_cfg.get("top_k", 40)
        self.max_tokens = int(main_cfg.get("max_tokens", 8192))
        self.thinking = str(main_cfg.get("enable_thinking", "none")).lower()

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

    def _get_thinking_param(self) -> Optional[Dict]:
        """将 enable_thinking 配置转换为 Anthropic thinking 参数

        Anthropic extended thinking API (Claude 3.7+):
        {"type": "enabled", "budget_tokens": int}

        enable_thinking 配置值:
        - "none" → 不设置（使用模型默认）
        - "true" → 启用 extended thinking，预算为 max_tokens 的一半
        - "false" → 不设置（Anthropic 默认不启用思考）

        Returns:
            dict 或 None: thinking 参数字典
        """
        if self.thinking == "true":
            # budget_tokens 至少 1024，且不超过 max_tokens - 1024
            budget = max(1024, self.max_tokens // 2)
            # 确保 max_tokens > budget_tokens
            if self.max_tokens <= budget:
                budget = self.max_tokens - 1024
            return {"type": "enabled", "budget_tokens": max(1024, budget)}
        return None

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
            converted_messages.append(
                {
                    "role": role,
                    "content": str(content),
                }
            )

        return system_prompt, converted_messages

    def generate_response(self, messages: List[Dict]) -> str:
        """生成非流式响应"""
        if self.client is None:
            error_message = "Anthropic 未初始化，请检查配置"
            logger.error(error_message)
            return error_message

        try:
            logger.debug(f"正在向 Anthropic 发送请求: {self.model_type}")

            system_prompt, converted_messages = self._extract_system_and_messages(
                messages
            )

            # 构建请求参数
            create_kwargs = {
                "model": self.model_type,
                "max_tokens": self.max_tokens,
                "messages": converted_messages,
            }

            # 添加 system prompt（如果有）
            if system_prompt:
                create_kwargs["system"] = system_prompt

            # 添加思考参数（extended thinking）
            thinking_param = self._get_thinking_param()
            if thinking_param:
                create_kwargs["thinking"] = thinking_param
                # 启用 extended thinking 时，max_tokens 需要包含 think budget
                create_kwargs["max_tokens"] = max(
                    self.max_tokens,
                    thinking_param["budget_tokens"] + 1024,
                )

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

            system_prompt, converted_messages = self._extract_system_and_messages(
                messages
            )

            # 构建请求参数
            create_kwargs = {
                "model": self.model_type,
                "max_tokens": self.max_tokens,
                "messages": converted_messages,
            }

            # 添加 system prompt
            if system_prompt:
                create_kwargs["system"] = system_prompt

            # 添加思考参数（extended thinking）
            thinking_param = self._get_thinking_param()
            if thinking_param:
                create_kwargs["thinking"] = thinking_param
                create_kwargs["max_tokens"] = max(
                    self.max_tokens,
                    thinking_param["budget_tokens"] + 1024,
                )

            # 添加可选参数
            if self.temperature > 0:
                create_kwargs["temperature"] = self.temperature
                if self.top_p and self.top_p < 1.0:
                    create_kwargs["top_p"] = self.top_p
                if self.top_k:
                    create_kwargs["top_k"] = self.top_k

            # 使用 messages.stream() 进行流式调用
            async with self.async_client.messages.stream(**create_kwargs) as stream:  # ty: ignore[invalid-argument-type]
                async for text in stream.text_stream:
                    yield text

        except Exception as e:
            logger.error(f"Anthropic 流式请求失败: {str(e)}")
            import traceback

            traceback.print_exc()
            raise

    def _convert_messages_with_tool_results(
        self, messages: List[Dict]
    ) -> tuple[Optional[str], List[Dict]]:
        """转换消息，处理 tool 和 tool_result 角色"""
        system_prompt = None
        converted_messages = []

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            if role == "system":
                system_prompt = str(content)
                continue

            # 转换 tool 角色为 Anthropic 格式
            if role == "tool":
                # Anthropic 使用 role="user"，content 包含 tool_result 块
                converted_messages.append(
                    {
                        "role": "user",
                        "content": [
                            {
                                "type": "tool_result",
                                "tool_use_id": msg.get("tool_call_id", ""),
                                "content": str(content),
                            }
                        ],
                    }
                )
                continue

            # 转换 tool_calls 消息格式
            if role == "assistant" and msg.get("tool_calls"):
                content_blocks = []
                if content:
                    content_blocks.append({"type": "text", "text": str(content)})
                for tc in msg.get("tool_calls", []):
                    content_blocks.append(
                        {
                            "type": "tool_use",
                            "id": tc.get("id", ""),
                            "name": tc.get("name", ""),
                            "input": tc.get("arguments", {}),
                        }
                    )
                converted_messages.append(
                    {"role": "assistant", "content": content_blocks}
                )
                continue

            converted_messages.append({"role": role, "content": str(content)})

        return system_prompt, converted_messages

    def generate_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> LLMResponse:
        """生成带工具调用的响应"""
        if self.client is None:
            error_message = "Anthropic 未初始化，请检查配置"
            logger.error(error_message)
            return LLMResponse(content=error_message)

        try:
            logger.debug(f"正在向 Anthropic 发送工具调用请求: {self.model_type}")

            system_prompt, converted_messages = (
                self._convert_messages_with_tool_results(messages)
            )

            # 转换工具定义
            anthropic_tools = ToolConverter.to_anthropic(tools)

            # 构建请求参数
            create_kwargs = {
                "model": self.model_type,
                "max_tokens": self.max_tokens,
                "messages": converted_messages,
                "tools": anthropic_tools,
            }

            if system_prompt:
                create_kwargs["system"] = system_prompt

            # 添加思考参数（extended thinking）
            thinking_param = self._get_thinking_param()
            if thinking_param:
                create_kwargs["thinking"] = thinking_param
                create_kwargs["max_tokens"] = max(
                    self.max_tokens,
                    thinking_param["budget_tokens"] + 1024,
                )

            if self.temperature > 0:
                create_kwargs["temperature"] = self.temperature
                if self.top_p and self.top_p < 1.0:
                    create_kwargs["top_p"] = self.top_p
                if self.top_k:
                    create_kwargs["top_k"] = self.top_k

            response = self.client.messages.create(**create_kwargs)
            # 解析响应内容
            text_content = ""
            tool_calls = []

            for block in response.content:
                if hasattr(block, "type"):
                    if block.type == "text":
                        text_content += block.text
                    elif block.type == "tool_use":
                        tool_calls.append(
                            ToolCall.from_anthropic(
                                {
                                    "id": block.id,
                                    "name": block.name,
                                    "input": block.input,
                                }
                            )
                        )

            return LLMResponse(content=text_content, tool_calls=tool_calls)

        except Exception as e:
            logger.error(f"Anthropic 工具调用请求失败: {str(e)}")
            raise

    async def generate_stream_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> AsyncGenerator[LLMResponse, None]:
        """生成带工具调用的流式响应"""
        if self.async_client is None:
            error_message = "Anthropic 未初始化，请检查配置"
            logger.error(error_message)
            raise ConnectionError(error_message)

        try:
            logger.debug(f"正在向 Anthropic 发送工具调用流式请求: {self.model_type}")

            system_prompt, converted_messages = (
                self._convert_messages_with_tool_results(messages)
            )

            anthropic_tools = ToolConverter.to_anthropic(tools)

            create_kwargs = {
                "model": self.model_type,
                "max_tokens": self.max_tokens,
                "messages": converted_messages,
                "tools": anthropic_tools,
            }

            if system_prompt:
                create_kwargs["system"] = system_prompt

            # 添加思考参数（extended thinking）
            thinking_param = self._get_thinking_param()
            if thinking_param:
                create_kwargs["thinking"] = thinking_param
                create_kwargs["max_tokens"] = max(
                    self.max_tokens,
                    thinking_param["budget_tokens"] + 1024,
                )

            if self.temperature > 0:
                create_kwargs["temperature"] = self.temperature
                if self.top_p and self.top_p < 1.0:
                    create_kwargs["top_p"] = self.top_p
                if self.top_k:
                    create_kwargs["top_k"] = self.top_k

            # Anthropic 支持流式工具调用，使用 messages.stream()
            # 流式响应中通过 content_block_start/content_block_delta 事件获取
            async with self.async_client.messages.stream(**create_kwargs) as stream:  # ty: ignore[invalid-argument-type]
                tool_calls = []
                current_tool_use_id = ""
                current_tool_name = ""
                current_tool_input_json = ""

                async for event in stream:
                    if event.type == "content_block_start":
                        block = event.content_block
                        if block.type == "text":
                            # 文本块开始
                            pass
                        elif block.type == "tool_use":
                            current_tool_use_id = block.id
                            current_tool_name = block.name
                            current_tool_input_json = ""

                    elif event.type == "content_block_delta":
                        delta = event.delta
                        if delta.type == "text_delta":
                            yield LLMResponse(content=delta.text, is_finished=False)
                        elif delta.type == "input_json_delta":
                            # 工具调用的输入参数（JSON 流式）
                            current_tool_input_json += delta.partial_json

                    elif event.type == "content_block_stop":
                        if current_tool_use_id:
                            # 工具调用块结束，解析完整参数
                            try:
                                import json

                                input_data = (
                                    json.loads(current_tool_input_json)
                                    if current_tool_input_json
                                    else {}
                                )
                            except json.JSONDecodeError:
                                input_data = {}

                            tool_calls.append(
                                ToolCall.from_anthropic(
                                    {
                                        "id": current_tool_use_id,
                                        "name": current_tool_name,
                                        "input": input_data,
                                    }
                                )
                            )
                            current_tool_use_id = ""
                            current_tool_name = ""
                            current_tool_input_json = ""

            yield LLMResponse(content="", tool_calls=tool_calls, is_finished=True)

        except Exception as e:
            logger.error(f"Anthropic 工具调用流式请求失败: {str(e)}")
            import traceback

            traceback.print_exc()
            raise
