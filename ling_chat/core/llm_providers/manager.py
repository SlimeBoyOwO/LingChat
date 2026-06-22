from typing import Awaitable, Callable, Dict, List

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.llm_providers.provider_factory import LLMProviderFactory
from ling_chat.core.llm_providers.tool_types import (
    ToolCall,
    ToolDefinition,
    ToolResult,
)
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
            provider_type, self.model_type, self.api_key, self.base_url
        )

    def process_message(self, messages: List[Dict]):
        return self.provider.generate_response(messages)

    async def process_message_stream(self, messages: List[Dict]):
        async for chunk in self.provider.generate_stream_response(messages):
            yield chunk

    async def process_message_with_tools(
        self,
        messages: List[Dict],
        tools: List[ToolDefinition],
        tool_executor: Callable[[ToolCall], Awaitable[str]],
        max_tool_calls: int = 10,
    ) -> str:
        """处理带工具的消息，自动完成多轮循环

        循环逻辑:
        1. 调用 provider.generate_with_tools()
        2. 如果有 tool_calls:
           - 并行执行所有 tool_executor
           - 将结果追加到 messages 作为 tool role
           - 重复步骤 1
        3. 直到没有 tool_calls 或达到 max_tool_calls
        4. 返回最终内容

        Args:
            messages: 消息列表
            tools: 工具定义列表
            tool_executor: 工具执行函数，接收 ToolCall 返回执行结果字符串
            max_tool_calls: 最大工具调用次数限制

        Returns:
            str: 最终响应内容
        """
        current_messages = messages.copy()
        tool_call_count = 0

        while tool_call_count < max_tool_calls:
            # 调用 LLM
            response = self.provider.generate_with_tools(current_messages, tools)

            # 如果没有工具调用，返回内容
            if not response.has_tool_calls:
                return response.content or ""

            # 并行执行工具调用
            import asyncio

            async def execute_tool(tc: ToolCall) -> ToolResult:
                try:
                    result = await tool_executor(tc)
                    return ToolResult(
                        tool_call_id=tc.id, name=tc.name, content=result, is_error=False
                    )
                except Exception as e:
                    logger.error(f"工具 {tc.name} 执行失败: {str(e)}")
                    return ToolResult(
                        tool_call_id=tc.id,
                        name=tc.name,
                        content=f"错误: {str(e)}",
                        is_error=True,
                    )

            # 并发执行所有工具
            tool_results = await asyncio.gather(
                *[execute_tool(tc) for tc in response.tool_calls]
            )

            # 构建 assistant 消息（包含 tool_calls）
            # OpenAI 格式要求 tool_calls 的结构为:
            # [{id, type, function: {name, arguments}}, ...]
            import json

            assistant_msg = {
                "role": "assistant",
                "content": response.content or "",
                "tool_calls": [
                    {
                        "id": tc.id,
                        "type": tc.type,
                        "function": {
                            "name": tc.name,
                            "arguments": json.dumps(tc.arguments, ensure_ascii=False),
                        },
                    }
                    for tc in response.tool_calls
                ],
            }
            current_messages.append(assistant_msg)

            # 添加 tool 结果消息
            for tr in tool_results:
                current_messages.append(tr.to_openai_message())

            tool_call_count += 1
            logger.debug(f"完成第 {tool_call_count} 轮工具调用")

        # 达到最大限制，最后一次调用获取最终回复
        final_response = self.provider.generate_with_tools(current_messages, tools)
        return final_response.content or ""

    async def process_message_stream_with_tools(
        self,
        messages: List[Dict],
        tools: List[ToolDefinition],
        tool_executor: Callable[[ToolCall], Awaitable[str]],
        max_tool_calls: int = 10,
    ):
        """处理带工具的流式消息，自动完成多轮循环

        Args:
            messages: 消息列表
            tools: 工具定义列表
            tool_executor: 工具执行函数
            max_tool_calls: 最大工具调用次数限制

        Yields:
            str: 流式响应内容
        """
        current_messages = messages.copy()
        tool_call_count = 0

        while tool_call_count < max_tool_calls:
            accumulated_content = ""
            tool_calls = []

            # 流式调用
            async for chunk in self.provider.generate_stream_with_tools(
                current_messages, tools
            ):
                if chunk.content:
                    accumulated_content += chunk.content
                    yield chunk.content

                if chunk.tool_calls:
                    tool_calls = chunk.tool_calls

                if chunk.is_finished:
                    break

            # 如果没有工具调用，完成
            if not tool_calls:
                return

            # 并行执行工具调用
            import asyncio

            async def execute_tool(tc: ToolCall) -> ToolResult:
                try:
                    result = await tool_executor(tc)
                    return ToolResult(
                        tool_call_id=tc.id, name=tc.name, content=result, is_error=False
                    )
                except Exception as e:
                    logger.error(f"工具 {tc.name} 执行失败: {str(e)}")
                    return ToolResult(
                        tool_call_id=tc.id,
                        name=tc.name,
                        content=f"错误: {str(e)}",
                        is_error=True,
                    )

            tool_results = await asyncio.gather(
                *[execute_tool(tc) for tc in tool_calls]
            )

            # 更新消息历史
            # OpenAI 格式要求 tool_calls 的结构为:
            # [{id, type, function: {name, arguments}}, ...]
            import json

            assistant_msg = {
                "role": "assistant",
                "content": accumulated_content,
                "tool_calls": [
                    {
                        "id": tc.id,
                        "type": tc.type,
                        "function": {
                            "name": tc.name,
                            "arguments": json.dumps(tc.arguments, ensure_ascii=False),
                        },
                    }
                    for tc in tool_calls
                ],
            }
            current_messages.append(assistant_msg)

            for tr in tool_results:
                current_messages.append(tr.to_openai_message())

            tool_call_count += 1

        # 最后一次调用获取最终回复
        async for chunk in self.provider.generate_stream_with_tools(
            current_messages, tools
        ):
            if chunk.content:
                yield chunk.content
            if chunk.is_finished:
                break
