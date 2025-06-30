from abc import ABC, abstractmethod
from typing import Dict, List

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