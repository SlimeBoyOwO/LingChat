from abc import ABC, abstractmethod
from typing import AsyncGenerator, Dict, List


class BaseLLMProvider(ABC):
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
    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """生成模型流式响应"""
        yield ""  # hack return type for async generator
