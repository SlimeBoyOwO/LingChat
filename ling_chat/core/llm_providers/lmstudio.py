import os
import json
from typing import AsyncGenerator, Dict, List, Any, Optional

import httpx

from ling_chat.core.logger import logger

from .base import BaseLLMProvider

# LM Studio API 文档：https://lmstudio.ai/docs/developer/rest/chat
# POST /api/v1/chat
class LMStudioProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.model_type = os.environ.get("LMSTUDIO_MODEL_TYPE", "")
        base_url = os.environ.get("LMSTUDIO_BASE_URL", "http://localhost:1234")
        self.base_url = base_url.replace("/v1", "")
        self.api_token = os.environ.get("LMSTUDIO_API_TOKEN", "")

    def initialize_client(self):
        """LM Studio 客户端在每次请求时创建，无需初始化"""
        pass

    def _get_headers(self):
        """获取请求头"""
        headers = {"Content-Type": "application/json"}
        if self.api_token:
            headers["Authorization"] = f"Bearer {self.api_token}"
        return headers

    def _get_http_client(self):
        """获取同步 HTTP 客户端"""
        return httpx.Client(
            base_url=self.base_url,
            headers=self._get_headers(),
            timeout=httpx.Timeout(120.0, connect=10.0)
        )

    def _get_async_client(self):
        """获取异步 HTTP 客户端"""
        return httpx.AsyncClient(
            base_url=self.base_url,
            headers=self._get_headers(),
            timeout=httpx.Timeout(120.0, connect=10.0)
        )

    def _build_input_messages(self, messages: List[Dict]) -> List[Dict]:
        """
        将 OpenAI 格式的 messages 转换为 LM Studio /api/v1/chat 的 input 格式

        OpenAI 格式：{"role": "user", "content": "..."}
        LM Studio input 格式：可以是字符串或对象数组
        """
        # 简单情况：如果只有一个用户消息，直接返回文本
        if len(messages) == 1 and messages[0].get("role") == "user":
            content = messages[0].get("content", "")
            # 检查是否包含图像
            if isinstance(content, list):
                # 多模态内容
                return self._convert_multimodal_content(content)
            else:
                # 纯文本
                return content

        # 多轮对话：转换为对象数组
        input_messages = []
        for msg in messages:
            content = msg.get("content", "")

            if isinstance(content, list):
                # 多模态内容（不需要 role 参数）
                input_messages.extend(self._convert_multimodal_content(content))
            else:
                # 纯文本
                input_messages.append({
                    "type": "text",
                    "content": content
                })

        return input_messages

    def _convert_multimodal_content(self, content: List[Dict]) -> List[Dict]:
        """
        转换多模态内容（文本 + 图像）

        LM Studio /api/v1/chat 端点 input 格式:
        - 文本：{"type": "text", "content": "..."}
        - 图像：{"type": "image", "data_url": "data:image/png;base64,..."}
        """
        result = []
        for item in content:
            if item.get("type") == "text":
                result.append({
                    "type": "text",
                    "content": item.get("text", "")
                })
            elif item.get("type") == "image_url":
                image_url = item.get("image_url", {})
                url = image_url.get("url", "") if isinstance(image_url, dict) else image_url
                result.append({
                    "type": "image",
                    "data_url": url
                })
        return result

    def _create_request_body(
        self,
        messages: List[Dict],
        stream: bool = False,
        **kwargs
    ) -> Dict[str, Any]:
        """
        构建 LM Studio /api/v1/chat 请求体

        参考 API 文档参数:
        - model: 必需
        - input: 必需，字符串或对象数组
        - system_prompt: 可选
        - stream: 可选，默认 false
        - temperature: 可选 [0,1]
        - top_p: 可选 [0,1]
        - top_k: 可选
        - min_p: 可选 [0,1]
        - repeat_penalty: 可选
        - max_output_tokens: 可选
        - reasoning: 可选 "off"|"low"|"medium"|"high"|"on"
        - context_length: 可选
        - store: 可选，默认 true
        """
        # 自动提取system消息并过滤messages
        system_prompt = None
        filtered_messages = []
        
        for msg in messages:
            if msg.get("role") == "system":
                system_prompt = msg.get("content", "")
            else:
                filtered_messages.append(msg)
        
        body = {
            "model": self.model_type,
            "input": self._build_input_messages(filtered_messages),
            "stream": stream,
        }

        if system_prompt:
            body["system_prompt"] = system_prompt

        temperature = os.environ.get("TEMPERATURE", "1.3")
        if temperature:
            body["temperature"] = float(temperature)

        top_p = os.environ.get("TOP_P", "0.9")
        if top_p:
            body["top_p"] = float(top_p)

        # 以下参数未使用
        max_tokens = os.environ.get("MAX_TOKENS")
        if max_tokens:
            body["max_output_tokens"] = int(max_tokens)

        top_k = os.environ.get("TOP_K")
        if top_k:
            body["top_k"] = int(top_k)

        min_p = os.environ.get("MIN_P")
        if min_p:
            body["min_p"] = float(min_p)

        repeat_penalty = os.environ.get("LMSTUDIO_REPEAT_PENALTY")
        if repeat_penalty:
            body["repeat_penalty"] = float(repeat_penalty)

        context_length = os.environ.get("LMSTUDIO_CONTEXT_LENGTH")
        if context_length:
            body["context_length"] = int(context_length)

        reasoning = os.environ.get("LMSTUDIO_REASONING")
        if reasoning:
            body["reasoning"] = reasoning

        return body

    async def _handle_stream_response(self, response: httpx.Response) -> AsyncGenerator[str, None]:
        """
        处理 SSE 流式响应

        LM Studio SSE 事件格式:
        event: <event_type>
        data: <JSON 数据>

        关键事件:
        - message.delta: 消息内容片段
        - message.end: 消息结束
        - chat.end: 聊天结束（包含完整响应）
        - error: 错误
        """
        current_event: Optional[str] = None
        current_data: Optional[Dict[str, Any]] = None

        async for line in response.aiter_lines():
            line = line.strip()
            if not line:
                # 空行表示事件块结束，处理已解析的事件
                if current_event is not None and current_data is not None:
                    async for chunk in self._process_sse_event(current_event, current_data):
                        yield chunk
                    current_event = None
                    current_data = None
                continue

            # 解析 SSE 格式
            if line.startswith("event:"):
                current_event = line.split(":", 1)[1].strip()
            elif line.startswith("data:"):
                data_str = line.split(":", 1)[1].strip()
                try:
                    current_data = json.loads(data_str)
                except json.JSONDecodeError:
                    logger.warning(f"无法解析 SSE 数据：{data_str}")
                    current_data = None
            # 忽略其他行

        # 处理最后一个事件
        if current_event is not None and current_data is not None:
            async for chunk in self._process_sse_event(current_event, current_data):
                yield chunk

    async def _process_sse_event(self, event_type: str, data: Dict[str, Any]) -> AsyncGenerator[str, None]:
        """
        处理单个 SSE 事件

        参数:
            event_type: 事件类型（message.delta, message.end, chat.end, error 等）
            data: 事件数据字典
        """
        if event_type == "message.delta":
            content = data.get("content", "")
            if content:
                yield content

        elif event_type == "error":
            error_info = data.get("error", {})
            error_msg = error_info.get("message", "未知错误")
            logger.error(f"LM Studio 流式传输错误：{error_msg}")
            raise Exception(f"LM Studio API 错误：{error_msg}")

        elif event_type == "chat.end":
            result = data.get("result", {})
            stats = result.get("stats", {})
            logger.debug(f"聊天完成。统计信息：{stats}")

    def generate_response(self, messages: List[Dict]) -> str:
        """生成 LM Studio 模型响应（非流式）

        LM Studio /api/v1/chat 响应格式:
        {
            "model_instance_id": "...",
            "output": [
                {"type": "message", "content": "..."},
                {"type": "reasoning", "content": "..."},
                {"type": "tool_call", ...}
            ],
            "stats": {...},
            "response_id": "resp_..."
        }
        """
        try:
            logger.debug("正在向 LM Studio /api/v1/chat 发送请求")

            request_body = self._create_request_body(messages, stream=False)

            with self._get_http_client() as client:
                response = client.post(
                    "/api/v1/chat",
                    json=request_body
                )

                # 检查响应状态
                response.raise_for_status()
                data = response.json()

                # 解析响应
                output = data.get("output", [])
                content_parts = []

                for item in output:
                    item_type = item.get("type")
                    if item_type == "message":
                        content_parts.append(item.get("content", ""))
                    elif item_type == "reasoning":
                        # 可选：收集推理内容
                        content_parts.append(item.get("content", ""))
                    elif item_type == "tool_call":
                        # 工具调用结果
                        tool_output = item.get("output", "")
                        if tool_output:
                            content_parts.append(f"[工具：{item.get('tool')}] {tool_output}")

                return "".join(content_parts)

        except httpx.HTTPStatusError as e:
            logger.error(f"LM Studio HTTP 错误：{e.response.status_code} - {e.response.text}")
            raise
        except httpx.RequestError as e:
            logger.error(f"LM Studio 请求错误：{str(e)}")
            raise
        except Exception as e:
            logger.error(f"LM Studio API 请求失败：{str(e)}")
            raise

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """生成 LM Studio 流式响应"""
        try:
            logger.debug("正在向 LM Studio /api/v1/chat 发送流式请求")

            request_body = self._create_request_body(messages, stream=True)

            async with self._get_async_client() as client:
                async with client.stream(
                    "POST",
                    "/api/v1/chat",
                    json=request_body
                ) as response:
                    response.raise_for_status()
                    async for chunk in self._handle_stream_response(response):
                        yield chunk

        except httpx.HTTPStatusError as e:
            logger.error(f"LM Studio HTTP 错误：{e.response.status_code} - {e.response.text}")
            raise
        except httpx.RequestError as e:
            logger.error(f"LM Studio 流式请求错误：{str(e)}")
            raise
        except Exception as e:
            logger.error(f"LM Studio 流式 API 请求失败：{str(e)}")
            raise
