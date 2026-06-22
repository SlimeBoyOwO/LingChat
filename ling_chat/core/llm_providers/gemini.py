import json
from typing import AsyncGenerator, Dict, List, Optional

import httpx

from ling_chat.configs.llm_config import llm_config
from ling_chat.core.llm_providers._http import build_httpx_client
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.llm_providers.tool_types import (
    LLMResponse,
    ToolCall,
    ToolConverter,
    ToolDefinition,
)
from ling_chat.core.logger import logger


# 文档：https://ai.google.dev/api
class GeminiProvider(BaseLLMProvider):
    def __init__(self, model_type: str = "", api_key: str = "", base_url: str = ""):
        super().__init__()
        main_cfg = llm_config.get_main_config()

        self.api_key = api_key or main_cfg.get("api_key", "")
        self.model_type = model_type or main_cfg.get("model", "gemini-2.5-flash")
        self.base_url = base_url or main_cfg.get(
            "base_url", "https://generativelanguage.googleapis.com/v1beta"
        )
        self.temperature = main_cfg.get("temperature", 1.0)
        self.top_p = main_cfg.get("top_p", 1.0)
        self.max_tokens = int(main_cfg.get("max_tokens", 8192))

        if not self.api_key:
            raise ValueError("需要Gemini API密钥！")

    def initialize_client(self):
        pass

    def _get_http_client(self):
        """获取HTTP客户端（同步），由全局 [network] 代理决定是否走代理"""
        timeout_config = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
        return build_httpx_client(timeout=timeout_config, base_url=self.base_url)

    def _get_async_http_client(self):
        """获取异步HTTP客户端，由全局 [network] 代理决定是否走代理"""
        timeout_config = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
        return build_httpx_client(
            async_client=True, timeout=timeout_config, base_url=self.base_url
        )

    def _convert_messages_to_contents(
        self, messages: List[Dict]
    ) -> tuple[Optional[str], List[Dict]]:
        """
        将OpenAI格式的消息转换为Gemini原生API格式

        支持工具调用相关消息的转换：
        - tool role → Gemini functionResponse 格式
        - assistant with tool_calls → Gemini model content 含 functionCall parts

        :param messages: OpenAI格式的消息列表
        :return: (system_instruction, contents) 元组
        """
        system_instruction = None
        contents = []

        for msg in messages:
            role = msg.get("role", "user")
            content = msg.get("content", "")

            # 处理system消息 - Gemini使用systemInstruction字段
            if role == "system":
                system_instruction = str(content)
                continue

            # 处理 tool role → Gemini functionResponse 格式
            # Gemini 文档：工具结果放在 role="user" 的 functionResponse part 中
            if role == "tool":
                tool_name = msg.get("name", "")
                tool_call_id = msg.get("tool_call_id", "")
                contents.append(
                    {
                        "role": "user",
                        "parts": [
                            {
                                "functionResponse": {
                                    "name": tool_name,
                                    "id": tool_call_id,
                                    "response": {"result": str(content)},
                                }
                            }
                        ],
                    }
                )
                continue

            # 处理 assistant + tool_calls → Gemini model content 含 functionCall parts
            if role == "assistant" and msg.get("tool_calls"):
                parts = []
                if content:
                    parts.append({"text": str(content)})
                for tc in msg.get("tool_calls", []):
                    parts.append(
                        {
                            "functionCall": {
                                "name": tc.get("name", ""),
                                "args": tc.get("arguments", {}),
                            }
                        }
                    )
                contents.append({"role": "model", "parts": parts})
                continue

            # 转换角色名称: user -> user, assistant/model -> model
            if role == "human":
                role = "user"
            elif role in ("assistant", "model"):
                role = "model"

            # 构建Gemini格式的content
            contents.append({"role": role, "parts": [{"text": str(content)}]})

        return system_instruction, contents

    def _build_request_body(self, messages: List[Dict], stream: bool = False) -> Dict:
        """构建Gemini API请求体"""
        system_instruction, contents = self._convert_messages_to_contents(messages)

        body = {
            "contents": contents,
            "generationConfig": {
                "temperature": self.temperature,
                "topP": self.top_p,
                "maxOutputTokens": self.max_tokens,
            },
        }

        # 添加system instruction（如果有）
        if system_instruction:
            body["systemInstruction"] = {"parts": [{"text": system_instruction}]}

        return body

    def generate_response(self, messages: List[Dict]) -> str:
        """生成Gemini模型响应（非流式）"""
        try:
            logger.debug(f"向Gemini API发送请求: {self.model_type}")

            body = self._build_request_body(messages, stream=False)
            url = f"{self.base_url}/models/{self.model_type}:generateContent?key={self.api_key}"

            with self._get_http_client() as client:
                response = client.post(url, json=body, timeout=60.0)

                if response.status_code != 200:
                    error_msg = (
                        f"Gemini API请求错误: {response.status_code} - {response.text}"
                    )
                    logger.error(error_msg)
                    raise Exception(error_msg)

                response_json = response.json()

                # 解析Gemini原生响应格式
                candidates = response_json.get("candidates", [])
                if not candidates:
                    logger.warning("Gemini API返回空candidates")
                    return ""

                content = candidates[0].get("content", {})
                parts = content.get("parts", [])

                # 拼接所有文本部分
                result_text = ""
                for part in parts:
                    if "text" in part:
                        result_text += part["text"]

                return result_text

        except Exception as e:
            logger.error(f"Gemini API请求错误: {str(e)}")
            raise

    async def generate_stream_response(
        self, messages: List[Dict]
    ) -> AsyncGenerator[str, None]:
        """生成Gemini流式响应

        :param messages: 消息列表
        :return: 生成器，逐个返回响应内容块
        """
        try:
            logger.debug(f"向Gemini模型发送流式请求: {self.model_type}")

            body = self._build_request_body(messages, stream=True)
            url = f"{self.base_url}/models/{self.model_type}:streamGenerateContent?key={self.api_key}&alt=sse"

            async with self._get_async_http_client() as client:
                async with client.stream(
                    "POST", url, json=body, timeout=60.0
                ) as response:
                    if response.status_code != 200:
                        await response.aread()
                        error_msg = f"Gemini流式API请求错误: {response.status_code} - {response.text}"
                        logger.error(error_msg)
                        raise Exception(error_msg)

                    async for line in response.aiter_lines():
                        if not line.strip():
                            continue

                        # SSE格式: data: {...}
                        if line.startswith("data: "):
                            chunk_data = line[6:]  # 移除 "data: " 前缀
                            if chunk_data == "[DONE]":
                                break

                            try:
                                chunk_json = json.loads(chunk_data)

                                # 解析Gemini流式响应格式
                                candidates = chunk_json.get("candidates", [])
                                if not candidates:
                                    continue

                                content = candidates[0].get("content", {})
                                parts = content.get("parts", [])

                                for part in parts:
                                    if "text" in part:
                                        yield part["text"]

                            except json.JSONDecodeError:
                                logger.warning(f"未能解析返回数据: {line}")
                                continue

        except Exception as e:
            logger.error(f"Gemini API流式请求失败: {str(e)}")
            raise

    def generate_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> LLMResponse:
        """生成带工具调用的响应"""
        try:
            logger.debug(f"向 Gemini API 发送工具调用请求: {self.model_type}")

            system_instruction, contents = self._convert_messages_to_contents(messages)

            # 转换工具定义
            gemini_tools = ToolConverter.to_gemini(tools)

            body = {
                "contents": contents,
                "tools": gemini_tools,
                "generationConfig": {
                    "temperature": self.temperature,
                    "topP": self.top_p,
                    "maxOutputTokens": self.max_tokens,
                },
            }

            if system_instruction:
                body["systemInstruction"] = {"parts": [{"text": system_instruction}]}

            url = f"{self.base_url}/models/{self.model_type}:generateContent?key={self.api_key}"

            with self._get_http_client() as client:
                response = client.post(url, json=body, timeout=60.0)

                if response.status_code != 200:
                    error_msg = f"Gemini API工具调用请求错误: {response.status_code} - {response.text}"
                    logger.error(error_msg)
                    raise Exception(error_msg)

                response_json = response.json()

                # 解析响应
                candidates = response_json.get("candidates", [])
                if not candidates:
                    logger.warning("Gemini API返回空candidates")
                    return LLMResponse(content="")

                content = candidates[0].get("content", {})
                parts = content.get("parts", [])

                text_content = ""
                tool_calls = []

                for part in parts:
                    if "text" in part:
                        text_content += part["text"]
                    elif "functionCall" in part:
                        fc = part["functionCall"]
                        tool_calls.append(
                            ToolCall.from_gemini(
                                {"name": fc.get("name", ""), "args": fc.get("args", {})}
                            )
                        )

                return LLMResponse(content=text_content, tool_calls=tool_calls)

        except Exception as e:
            logger.error(f"Gemini API工具调用请求错误: {str(e)}")
            raise

    async def generate_stream_with_tools(
        self, messages: List[Dict], tools: List[ToolDefinition]
    ) -> AsyncGenerator[LLMResponse, None]:
        """生成带工具调用的流式响应"""
        # Gemini 流式工具调用较复杂，先使用非流式实现
        try:
            response = self.generate_with_tools(messages, tools)
            yield response
        except Exception as e:
            logger.error(f"Gemini API流式工具调用请求失败: {str(e)}")
            raise
