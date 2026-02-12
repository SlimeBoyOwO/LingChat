import json
import os
from typing import AsyncGenerator, Dict, List

import httpx

from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


def _normalize_base_url(raw: str) -> str:
    raw = (raw or "").strip()
    if not raw:
        return "http://localhost:11434"
    if raw.startswith("http://") or raw.startswith("https://"):
        return raw.rstrip("/")
    return f"http://{raw}".rstrip("/")


class OllamaProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.base_url = _normalize_base_url(
            os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434")
        )
        self.model_type = (
            os.environ.get("OLLAMA_MODEL")
            or os.environ.get("MODEL_TYPE")
            or "llama3"
        )
        self._timeout = httpx.Timeout(timeout=30.0, connect=5.0)

    def initialize_client(self):
        pass

    def generate_response(self, messages: List[Dict]) -> str:
        """生成Ollama模型响应"""
        try:
            logger.info(f"Sending request to Ollama API: {self.base_url}/api/chat")

            payload = {
                "model": self.model_type,
                "messages": messages,
                "stream": False
            }

            with httpx.Client(timeout=self._timeout) as client:
                response = client.post(
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
            logger.info(f"正在给 Ollama 发送流式请求: {self.base_url}/api/chat")

            payload = {
                "model": self.model_type,
                "messages": messages,
                "stream": True
            }

            async with httpx.AsyncClient(timeout=self._timeout) as client:
                async with client.stream(
                    "POST",
                    f"{self.base_url}/api/chat",
                    json=payload,
                ) as response:
                    if response.status_code != 200:
                        body = await response.aread()
                        text = ""
                        try:
                            text = body.decode("utf-8", errors="replace")
                        except Exception:
                            text = str(body)
                        error_msg = f"Ollama 流式返回了错误: {response.status_code} - {text}"
                        logger.error(error_msg)
                        raise Exception(error_msg)

                    async for line in response.aiter_lines():
                        if line.strip():  # 确保不是空行
                            try:
                                chunk_json = json.loads(line)
                                content = chunk_json.get("message", {}).get("content", "")
                                if content:
                                    yield content
                            except json.JSONDecodeError:
                                logger.warning(f"无法解析的响应块: {line}")
                                continue

        except Exception as e:
            logger.error(f"Ollama 流式调用失败: {str(e)}")
            raise
