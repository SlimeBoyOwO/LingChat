# neochat/llm/clients/base.py
from abc import ABC, abstractmethod
from typing import List, Dict, Generator, Any

class BaseLLMClient(ABC):
    """
    所有 LLM 客户端的抽象基类。
    定义了与大语言模型交互的标准接口。
    """
    def __init__(self, config: Dict[str, Any]):
        """
        使用提供商的特定配置初始化客户端。
        :param config: 来自 config.yaml 的提供商配置字典。
        """
        self.config = config
        self.default_model = config.get("default_model")
        self.default_parameters = config.get("default_parameters", {})

    @abstractmethod
    def chat_completion(
        self,
        messages: List[Dict[str, Any]],
        model: str = None,
        stream: bool = True,
        **kwargs
    ) -> Generator[str, None, None]:
        """
        执行聊天补全请求。

        :param messages: OpenAI 格式的消息列表。
                         对于多模态，content 可以是一个列表，例如:
                         [{"type": "text", "text": "图片里有什么?"},
                          {"type": "image_url", "image_url": {"url": "data:image/jpeg;base64,..."}}]
        :param model: 要使用的模型名称。如果为 None，则使用配置中的默认模型。
        :param stream: 是否以流式方式返回响应。
        :param kwargs: 其他传递给 API 的参数 (如 temperature, max_tokens)。
        :return: 一个生成器，逐块产生响应的文本内容。
        """
        pass