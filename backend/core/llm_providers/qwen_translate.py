import os
from openai import OpenAI
from .base import BaseLLMProvider
from typing import Dict, List, AsyncGenerator
from core.logger import logger

class QwenTranslateProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.client = None
        self.model_type = None
        self.initialize_client()
    
    def initialize_client(self):
        """初始化Qwen客户端"""
        api_key = os.environ.get("TRANSLATE_API_KEY")
        base_url = os.environ.get("QWEN_BASE_URL", "https://dashscope.aliyuncs.com/compatible-mode/v1")
        
        if not api_key:
            error_message = "没有配置TRANSLATE_API_KEY，请检查配置"
            logger.warning(error_message)
            self.client = None
            return
        
        self.client = OpenAI(api_key=api_key, base_url=base_url)
        self.model_type = os.environ.get("QWEN_MODEL_TYPE", "qwen-mt-plus")

        logger.info("Qwen翻译模型初始化完毕！")
    
    def generate_response(self, messages: List[Dict]) -> str:
        """生成Qwen模型响应"""
        if self.client is None or self.model_type is None:
            error_message = "Qwen翻译模型未初始化，请检查配置"
            logger.error(error_message)
            return error_message
            
        # 只提取最后助手的回答
        filtered_messages = []
        for msg in reversed(messages):
            if msg.get("role") == "user":
                filtered_messages = [msg]
                break

        translation_options = {
            "source_lang": "auto",
            "target_lang": "ja",
            "terms": [
                {
                    "source": "<",
                    "target": "<"
                },
                {
                    "source": ">",
                    "target": ">"
                }
                    ]
        }

        try:
            logger.debug(f"正在对Qwen翻译模型发送请求: {self.model_type}")
            response = self.client.chat.completions.create(
                model=str(self.model_type),
                messages=filtered_messages,  # type: ignore
                stream=False,
                extra_body={
                    "translation_options": translation_options
                }
            )
            return response.choices[0].message.content
            
        except Exception as e:
            logger.error(f"Qwen翻译模型请求失败: {str(e)}")
            raise
    
    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """
        生成流式响应
        :param messages: 消息列表
        :return: 返回一个生成器，每次迭代返回一个chunk
        """
        if self.client is None or self.model_type is None:
            error_message = "Qwen翻译模型未初始化，请检查配置"
            logger.error(error_message)
            yield error_message
            return
        
        # 只提取最后助手的回答
        filtered_messages = []
        for msg in reversed(messages):
            if msg.get("role") == "user":
                filtered_messages = [msg]
                break

        translation_options = {
            "source_lang": "auto",
            "target_lang": "ja",
            "terms": [
                {
                    "source": "<",
                    "target": "<"
                },
                {
                    "source": ">",
                    "target": ">"
                }
                    ]
        }

        try:
            logger.debug(f"正在对Qwen翻译模型发送流式请求: {self.model_type}")
            stream = self.client.chat.completions.create(
                model=str(self.model_type),
                messages=filtered_messages,  # type: ignore
                stream=True,
                extra_body={
                    "translation_options": translation_options
                }
            )
            
            for chunk in stream:
                if chunk.choices[0].delta.content is not None:
                    yield chunk.choices[0].delta.content
                    
        except Exception as e:
            logger.error(f"Qwen翻译模型流式请求失败: {str(e)}")
            raise