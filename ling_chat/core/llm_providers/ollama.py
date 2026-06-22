import json
from typing import AsyncGenerator, Dict, List

import httpx

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.llm_providers._http import build_httpx_client
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.llm_providers.tool_types import (
    LLMResponse,
    ToolCall,
    ToolDefinition,
)
from ling_chat.core.logger import logger


def _normalize_base_url(raw: str) -> str:
    raw = (raw or "").strip()
    if not raw:
        return "http://localhost:11434"
    if raw.startswith("http://") or raw.startswith("https://"):
        return raw.rstrip("/")
    return f"http://{raw}".rstrip("/")


class OllamaProvider(BaseLLMProvider):
    def __init__(self, model_type: str = "", api_key: str = "", base_url: str = ""):
        super().__init__()
        self.model_type = model_type or "llama3"
        self.base_url = _normalize_base_url(base_url or "http://localhost:11434")
        main_cfg = llm_config.get_main_config()
        self._timeout = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
        self.temperature = main_cfg.get("temperature", 1.3)
        self.top_p = main_cfg.get("top_p", 0.9)
        self.max_tokens = int(main_cfg.get("max_tokens", 8192))

    def initialize_client(self):
        pass

    def generate_response(self, messages: List[Dict]) -> str:
        """生成Ollama模型响应"""
        try:
            logger.info(f"Sending request to Ollama API: {self.base_url}/api/chat")

            payload = {
                "model": self.model_type,
                "messages": messages,
                "options": {
                    "temperature": self.temperature,
                    "top_p": self.top_p,
                    "num_predict": self.max_tokens,
                },
                "stream": False,
            }

            with build_httpx_client(
                timeout=self._timeout, base_url=self.base_url
            ) as client:
                response = client.post(f"{self.base_url}/api/chat", json=payload)

                if response.status_code != 200:
                    error_msg = f"Ollama API returned error: {response.status_code} - {response.text}"
                    logger.error(error_msg)
                    raise Exception(error_msg)

                response_json = response.json()
                return response_json.get("message", {}).get("content", "")

        except Exception as e:
            logger.error(f"Ollama API call failed: {str(e)}")
            raise

    async def generate_stream_response(
        self, messages: List[Dict]
    ) -> AsyncGenerator[str, None]:
        """生成Ollama流式响应
        :param messages: 消息列表
        :return: 返回一个生成器，每次迭代返回一个内容块
        """
        try:
            logger.info(f"正在给 Ollama 发送流式请求: {self.base_url}/api/chat")

            payload = {
                "model": self.model_type,
                "messages": messages,
                "options": {
                    "temperature": self.temperature,
                    "top_p": self.top_p,
                    "num_predict": self.max_tokens,
                },
                "stream": True,
            }

            async with build_httpx_client(
                async_client=True, timeout=self._timeout, base_url=self.base_url
            ) as client:
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
                        error_msg = (
                            f"Ollama 流式返回了错误: {response.status_code} - {text}"
                        )
                        logger.error(error_msg)
                        raise Exception(error_msg)

                    async for line in response.aiter_lines():
                        if line.strip():  # 确保不是空行
                            try:
                                chunk_json = json.loads(line)
                                content = chunk_json.get("message", {}).get(
                                    "content", ""
                                )
                                if content:
                                    yield content
                            except json.JSONDecodeError:
                                logger.warning(f"无法解析的响应块: {line}")
                                continue

        except Exception as e:
            logger.error(f"Ollama 流式调用失败: {str(e)}")
            raise

    def generate_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> LLMResponse:
        """生成带工具调用的响应

        Ollama 从 0.3.0 开始支持 OpenAI 兼容的工具调用格式
        """
        try:
            logger.debug(f"向 Ollama 发送工具调用请求: {self.model_type}")

            # 转换工具定义为 OpenAI 格式
            tools_openai = [tool.model_dump() for tool in tools]

            payload = {
                "model": self.model_type,
                "messages": messages,
                "tools": tools_openai,
                "options": {
                    "temperature": self.temperature,
                    "top_p": self.top_p,
                    "num_predict": self.max_tokens,
                },
                "stream": False,
            }

            with build_httpx_client(
                timeout=self._timeout, base_url=self.base_url
            ) as client:
                response = client.post(f"{self.base_url}/api/chat", json=payload)

                if response.status_code != 200:
                    error_msg = f"Ollama API工具调用返回错误: {response.status_code} - {response.text}"
                    logger.error(error_msg)
                    raise Exception(error_msg)

                response_json = response.json()
                message = response_json.get("message", {})

                # 解析工具调用
                # Ollama /api/chat 响应格式中 tool_calls 的结构为:
                # [{function: {name, arguments}}, ...]
                # 其中 arguments 是对象而非字符串
                tool_calls = []
                if "tool_calls" in message:
                    for tc in message["tool_calls"]:
                        func = tc.get("function", {})
                        # Ollama 的 arguments 是对象，直接传入
                        # OpenAI 的 arguments 是 JSON 字符串
                        # from_openai 方法会正确处理两种情况
                        tool_calls.append(
                            ToolCall.from_openai(
                                {
                                    "id": tc.get("id", ""),
                                    "type": tc.get("type", "function"),
                                    "function": func,
                                }
                            )
                        )

                return LLMResponse(
                    content=message.get("content", ""), tool_calls=tool_calls
                )

        except Exception as e:
            logger.error(f"Ollama API工具调用失败: {str(e)}")
            raise

    async def generate_stream_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> AsyncGenerator[LLMResponse, None]:
        """生成带工具调用的流式响应"""
        try:
            logger.debug(f"向 Ollama 发送工具调用流式请求: {self.model_type}")

            tools_openai = [tool.model_dump() for tool in tools]

            payload = {
                "model": self.model_type,
                "messages": messages,
                "tools": tools_openai,
                "options": {
                    "temperature": self.temperature,
                    "top_p": self.top_p,
                    "num_predict": self.max_tokens,
                },
                "stream": True,
            }

            async with build_httpx_client(
                async_client=True, timeout=self._timeout, base_url=self.base_url
            ) as client:
                async with client.stream(
                    "POST",
                    f"{self.base_url}/api/chat",
                    json=payload,
                ) as response:
                    if response.status_code != 200:
                        body = await response.aread()
                        text = body.decode("utf-8", errors="replace")
                        error_msg = f"Ollama 流式工具调用返回错误: {response.status_code} - {text}"
                        logger.error(error_msg)
                        raise Exception(error_msg)

                    accumulated_content = ""
                    tool_calls_data = {}

                    async for line in response.aiter_lines():
                        if line.strip():
                            try:
                                chunk_json = json.loads(line)
                                message = chunk_json.get("message", {})
                                content = message.get("content", "")

                                if content:
                                    accumulated_content += content
                                    yield LLMResponse(
                                        content=content, is_finished=False
                                    )

                                # 处理工具调用
                                if "tool_calls" in message:
                                    for tc in message["tool_calls"]:
                                        tc_id = tc.get("id", "")
                                        if tc_id not in tool_calls_data:
                                            tool_calls_data[tc_id] = tc

                            except json.JSONDecodeError:
                                logger.warning(f"无法解析的响应块: {line}")
                                continue

                    # 构建最终工具调用
                    tool_calls = []
                    for tc in tool_calls_data.values():
                        tool_calls.append(ToolCall.from_openai(tc))

                    yield LLMResponse(
                        content="", tool_calls=tool_calls, is_finished=True
                    )

        except Exception as e:
            logger.error(f"Ollama 流式工具调用失败: {str(e)}")
            raise
