from abc import ABC, abstractmethod
from typing import Dict, List, Union

class BaseLLMProvider(ABC):
    """LLM提供商的抽象基类"""
    
    @abstractmethod
    def __init__(self, config: Dict):
        pass
    
    @abstractmethod
    def chat_completion(self, messages: List[Dict]) -> Union[str, None]:
        pass
    
    @property
    @abstractmethod
    def model_type(self) -> str:
        pass

class LLMError(Exception):
    """自定义LLM异常基类"""
    pass