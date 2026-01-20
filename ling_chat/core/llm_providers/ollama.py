import json
import os
from typing import AsyncGenerator, Dict, List

import requests

from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class OllamaProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.base_url = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434")
        self.model_type = os.environ.get("OLLAMA_MODEL", "llama3")

    def initialize_client(self):
        pass

    def generate_response(self, messages: List[Dict]) -> str:
        """生成Ollama模型响应"""
        try:
            logger.debug(f"Sending request to Ollama API: {self.base_url}/api/chat")

            payload = {
                "model": self.model_type,
                "messages": messages,
                "stream": False
            }

            response = requests.post(
                f"{self.base_url}/api/chat",
                json=payload
            )

            if response.status_code != 200:
                error_msg = f"Ollama API returned error: {response.status_code} - {response.text}"
                logger.error(error_msg)
                raise Exception(error_msg)

            response_json = response.json()
            return response_json.get("message", {}).get("content", "")

        except Exception as e:
            logger.error(f"Ollama API call failed: {str(e)}")
            raise

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """生成Ollama流式响应
        :param messages: 消息列表
        :return: 返回一个生成器，每次迭代返回一个内容块
        """
        try:
            logger.debug(f"正在给 Ollama 发送流式请求: {self.base_url}/api/chat")

            payload = {
                "model": self.model_type,
                "messages": messages,
                "stream": True
            }

            with requests.post(
                    f"{self.base_url}/api/chat",
                    json=payload,
                    stream=True
            ) as response:
                if response.status_code != 200:
                    error_msg = f"Ollama 流式返回了错误: {response.status_code} - {response.text}"
                    logger.error(error_msg)
                    raise Exception(error_msg)

                for chunk in response.iter_lines():
                    if chunk:
                        decoded_chunk = chunk.decode('utf-8')
                        if decoded_chunk.strip():  # 确保不是空行
                            try:
                                chunk_json = json.loads(decoded_chunk)
                                content = chunk_json.get("message", {}).get("content", "")
                                if content:
                                    yield content
                            except json.JSONDecodeError:
                                logger.warning(f"无法解析的响应块: {decoded_chunk}")
                                continue

        except Exception as e:
            logger.error(f"Ollama 流式调用失败: {str(e)}")
            raise
