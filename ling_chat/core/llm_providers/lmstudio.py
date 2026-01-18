import os
from typing import AsyncGenerator, Dict, List

from openai import OpenAI

from ling_chat.core.logger import logger

from .base import BaseLLMProvider


class LMStudioProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.client = None
        self.model_type = os.environ.get("LMSTUDIO_MODEL_TYPE", "")
        self.base_url = os.environ.get("LMSTUDIO_BASE_URL", "http://localhost:1234/v1")
        self.initialize_client()

    def initialize_client(self):
        """初始化LM Studio客户端"""
        try:
            self.client = OpenAI(
                base_url=self.base_url,
                api_key="lm-studio"  # LM Studio通常不需要真实API key
            )
            logger.info("LM Studio client initialized successfully")
        except Exception as e:
            logger.error(f"Failed to initialize LM Studio client: {str(e)}")
            raise

    def _create_api_request(self, messages: List[Dict], stream: bool = False):
        """构建统一的API请求参数"""
        return {
            "model": self.model_type,
            "messages": messages,
            "stream": stream,
            "temperature": 0.7,  # 可配置化
            "max_tokens": 2048  # 可配置化
        }

    def generate_response(self, messages: List[Dict]) -> str:
        """生成LM Studio模型响应"""
        try:
            logger.debug("Sending request to LM Studio model")
            response = self.client.chat.completions.create(
                **self._create_api_request(messages, stream=False)
            )
            return response.choices[0].message.content

        except Exception as e:
            logger.error(f"LM Studio API request failed: {str(e)}")
            raise

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """生成LM Studio流式响应"""
        try:
            logger.debug("Sending streaming request to LM Studio model")
            stream = self.client.chat.completions.create(
                **self._create_api_request(messages, stream=True)
            )

            for chunk in stream:
                if chunk.choices[0].delta.content is not None:
                    yield chunk.choices[0].delta.content

        except Exception as e:
            logger.error(f"LM Studio streaming API request failed: {str(e)}")
            raise
