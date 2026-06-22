from abc import ABC, abstractmethod
from typing import AsyncGenerator, Dict, List

from ling_chat.core.llm_providers.tool_types import LLMResponse, ToolDefinition


class BaseLLMProvider(ABC):
    @abstractmethod
    def __init__(self):
        pass

    @abstractmethod
    def initialize_client(self):
        """初始化客户端连接"""
        pass

    @abstractmethod
    def generate_response(self, messages: List[Dict]) -> str:
        """生成模型响应"""
        pass

    @abstractmethod
    async def generate_stream_response(
        self, messages: List[Dict]
    ) -> AsyncGenerator[str, None]:
        """生成模型流式响应"""
        yield ""  # hack return type for async generator

    @abstractmethod
    def generate_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> LLMResponse:
        """生成带工具调用的响应

        Args:
            messages: 消息列表
            tools: 工具定义列表

        Returns:
            LLMResponse: 包含内容和工具调用的响应
        """
        pass

    @abstractmethod
    async def generate_stream_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> AsyncGenerator[LLMResponse, None]:
        """生成带工具调用的流式响应

        Args:
            messages: 消息列表
            tools: 工具定义列表

        Yields:
            LLMResponse: 包含内容和工具调用的响应片段
        """
        yield LLMResponse(content="")
