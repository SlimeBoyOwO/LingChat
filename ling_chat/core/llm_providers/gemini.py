import json
import os
from typing import AsyncGenerator, Dict, List

import httpx

from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class GeminiProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.api_key = os.environ.get("GEMINI_API_KEY")
        self.base_url = "https://generativelanguage.googleapis.com/v1beta/openai"
        self.model_type = os.environ.get("GEMINI_MODEL_TYPE", "gemini-2.5-flash")
        self.proxy_url = os.environ.get("GEMINI_PROXY_URL")
        self.temperature = float(os.environ.get("TEMPERATURE", 1.0))
        self.top_p = float(os.environ.get("TOP_P", 1.0))

        if not self.api_key:
            raise ValueError("需要Gemini API密钥！")
        
    def initialize_client(self):
        pass

    def _get_headers(self):
        """获取请求头"""
        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.api_key}"
        }
        return headers

    def _get_http_client(self):
        """获取HTTP客户端，支持代理"""
        timeout_config = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
        if self.proxy_url and self.proxy_url.strip():
            return httpx.Client(proxy=self.proxy_url, timeout=timeout_config)
        return httpx.Client(timeout=timeout_config)

    def _format_messages(self, messages: List[Dict]) -> List[Dict]:
        """格式化消息为Gemini API兼容格式

        Gemini API支持OpenAI兼容的消息格式，但需要确保role是有效的：
        - system, user, assistant
        """
        formatted_messages = []
        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            # 确保角色是Gemini API接受的
            if role == "human":
                role = "user"
            elif role == "model":
                role = "assistant"

            formatted_messages.append({
                "role": role,
                "content": str(content)
            })
        return formatted_messages

    def generate_response(self, messages: List[Dict]) -> str:
        """生成Gemini模型响应（非流式）"""
        try:
            logger.debug(f"向Gemini API发送请求: {self.model_type}")

            formatted_messages = self._format_messages(messages)

            payload = {
                "model": self.model_type,
                "messages": formatted_messages,
                "stream": False,
                "temperature": self.temperature,
                "top_p": self.top_p
            }

            with self._get_http_client() as client:
                response = client.post(
                    f"{self.base_url}/chat/completions",
                    headers=self._get_headers(),
                    json=payload,
                    timeout=30.0
                )

                if response.status_code != 200:
                    error_msg = f"Gemini API请求错误: {response.status_code} - {response.text}"
                    logger.error(error_msg)
                    raise Exception(error_msg)

                response_json = response.json()
                return response_json.get("choices", [{}])[0].get("message", {}).get("content", "")

        except Exception as e:
            logger.error(f"Gemini API请求错误: {str(e)}")
            raise

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """生成Gemini流式响应

        :param messages: 消息列表
        :return: 生成器，逐个返回响应内容块
        """
        try:
            logger.debug(f"向Gemini模型发送流式请求: {self.model_type}")

            formatted_messages = self._format_messages(messages)

            payload = {
                "model": self.model_type,
                "messages": formatted_messages,
                "stream": True,
                "temperature": self.temperature,
                "top_p": self.top_p
            }

            # 设置完整的超时配置：connect=20秒, read=60秒, write=20秒, pool=20秒
            timeout_config = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
            # 只有当代理URL既不为None也不为空字符串时才使用代理
            proxy_config = self.proxy_url if self.proxy_url and self.proxy_url.strip() else None
            async with httpx.AsyncClient(proxy=proxy_config, timeout=timeout_config) as client:
                async with client.stream(
                    'POST',
                    f"{self.base_url}/chat/completions",
                    headers=self._get_headers(),
                    json=payload,
                    timeout=60.0
                ) as response:
                    if response.status_code != 200:
                        await response.aread()
                        error_msg = f"Gemini流式API请求错误: {response.status_code} - {response.text}"
                        logger.error(error_msg)
                        raise Exception(error_msg)

                    async for line in response.aiter_lines():
                        if line.strip() and line.startswith("data: "):
                            chunk_data = line[6:]  # 移除 "data: " 前缀
                            if chunk_data == "[DONE]":
                                break

                            try:
                                chunk_json = json.loads(chunk_data)
                                if chunk_json.get("object") == "chat.completion.chunk":
                                    choices = chunk_json.get("choices", [])
                                    if choices:
                                        delta = choices[0].get("delta", {})
                                        content = delta.get("content", "")
                                        if content:
                                            yield content
                            except json.JSONDecodeError:
                                logger.warning(f"未能解析返回数据: {line}")
                                continue

        except Exception as e:
            logger.error(f"Gemini API流式请求失败: {str(e)}")
            raise