import os
from typing import AsyncGenerator, Dict, List

import httpx
from anthropic import Anthropic, AsyncAnthropic

from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class KimiCodeProvider(BaseLLMProvider):
    """Kimi Code Provider，使用 Anthropic 兼容格式

    参考 AstrBot 的 ProviderKimiCode 实现。
    Kimi Code API (https://api.kimi.com/coding) 使用 Anthropic Messages API 格式，
    而非 OpenAI 的 Chat Completions 格式。
    """

    KIMI_CODE_API_BASE = "https://api.kimi.com/coding"
    KIMI_CODE_DEFAULT_MODEL = "kimi-for-coding"
    KIMI_CODE_USER_AGENT = "claude-code/0.1.0"

    def __init__(self, model_type: str, api_key: str, base_url: str):
        super().__init__()
        self.api_key = api_key
        # Kimi Code 使用固定的 coding endpoint
        if base_url and base_url != self.KIMI_CODE_API_BASE:
            logger.warning(
                f"Kimi Code 使用固定 API 端点 {self.KIMI_CODE_API_BASE}，"
                f"传入的 base_url ({base_url}) 已被忽略"
            )
        self.base_url = self.KIMI_CODE_API_BASE
        self.model_type = model_type or self.KIMI_CODE_DEFAULT_MODEL
        self.temperature = float(os.environ.get("TEMPERATURE", 1.3))
        self.top_p = float(os.environ.get("TOP_P", 0.9))
        self.max_tokens = int(os.environ.get("MAX_TOKENS", "8192"))

        if (not api_key) or api_key == "sk-114514":
            logger.warning("Kimi Code 未初始化：CHAT_API_KEY 为空或为占位值。")
            self.client = None
            self.async_client = None
            return

        self._timeout = httpx.Timeout(connect=20.0, read=60.0, write=20.0, pool=20.0)
        default_headers = {"User-Agent": self.KIMI_CODE_USER_AGENT}

        self.client = Anthropic(
            api_key=api_key,
            base_url=self.base_url,
            timeout=self._timeout,
            default_headers=default_headers,
        )
        self.async_client = AsyncAnthropic(
            api_key=api_key,
            base_url=self.base_url,
            timeout=self._timeout,
            default_headers=default_headers,
        )
        logger.info("Kimi Code 大模型初始化完毕！")

    def initialize_client(self):
        """Kimi Code 客户端在 __init__ 时已初始化，无需额外操作"""
        pass

    @staticmethod
    def _convert_messages(messages: List[Dict]) -> tuple[str, List[Dict]]:
        """将 OpenAI 格式的 messages 转换为 Anthropic 格式

        Returns:
            system_prompt: 系统提示内容
            anthropic_messages: Anthropic 格式的消息列表
        """
        system_prompt = ""
        anthropic_messages = []

        for message in messages:
            role = message.get("role", "")
            content = message.get("content", "")

            if role == "system":
                system_prompt = content or ""
            elif role == "assistant":
                blocks = []
                if isinstance(content, str) and content.strip():
                    blocks.append({"type": "text", "text": content})
                elif isinstance(content, list):
                    for part in content:
                        if isinstance(part, dict):
                            blocks.append(part)
                        else:
                            blocks.append({"type": "text", "text": str(part)})
                anthropic_messages.append({"role": "assistant", "content": blocks})
            elif role == "user":
                blocks = []
                if isinstance(content, str):
                    blocks.append({"type": "text", "text": content})
                elif isinstance(content, list):
                    for part in content:
                        if isinstance(part, dict):
                            pt = part.get("type", "")
                            if pt == "image_url":
                                image_url_data = part.get("image_url", {})
                                url = image_url_data.get("url", "")
                                if url.startswith("data:"):
                                    try:
                                        import base64

                                        _, base64_data = url.split(",", 1)
                                        image_bytes = base64.b64decode(base64_data)
                                        media_type = "image/jpeg"
                                        if image_bytes[:8] == b"\x89PNG\r\n\x1a\n":
                                            media_type = "image/png"
                                        elif image_bytes[:3] == b"GIF":
                                            media_type = "image/gif"
                                        elif image_bytes[:2] == b"BM":
                                            media_type = "image/bmp"
                                        elif image_bytes[:4] == b"RIFF":
                                            media_type = "image/webp"
                                        blocks.append(
                                            {
                                                "type": "image",
                                                "source": {
                                                    "type": "base64",
                                                    "media_type": media_type,
                                                    "data": base64_data,
                                                },
                                            }
                                        )
                                    except Exception:
                                        blocks.append(
                                            {
                                                "type": "text",
                                                "text": "[图片解析失败]",
                                            }
                                        )
                                else:
                                    blocks.append(
                                        {
                                            "type": "text",
                                            "text": f"[图片: {url}]",
                                        }
                                    )
                            else:
                                blocks.append(part)
                        else:
                            blocks.append({"type": "text", "text": str(part)})
                anthropic_messages.append({"role": "user", "content": blocks})
            else:
                anthropic_messages.append({"role": role, "content": content})

        return system_prompt, anthropic_messages

    def _build_payload(self, messages: List[Dict]) -> Dict:
        """构建 Anthropic API 请求参数"""
        system_prompt, anthropic_messages = self._convert_messages(messages)

        # Anthropic 要求 user/assistant 必须交替出现
        # 简单处理：如果第一条不是 user，在前面插入一条空的 user 消息
        if anthropic_messages and anthropic_messages[0].get("role") != "user":
            anthropic_messages.insert(
                0, {"role": "user", "content": [{"type": "text", "text": "..."}]}
            )

        payload = {
            "model": self.model_type,
            "messages": anthropic_messages,
            "max_tokens": self.max_tokens,
        }

        if system_prompt:
            payload["system"] = system_prompt

        return payload

    def generate_response(self, messages: List[Dict]) -> str:
        """生成模型响应（非流式）"""
        if self.client is None:
            error_message = "Kimi Code 未初始化，请检查配置"
            logger.error(error_message)
            return error_message

        try:
            logger.debug(f"正在对 Kimi Code 发送请求: {self.model_type}")
            payload = self._build_payload(messages)

            response = self.client.messages.create(**payload, stream=False)

            # 提取文本内容
            text_parts = []
            for block in response.content:
                if block.type == "text":
                    text_parts.append(block.text)

            return "".join(text_parts)

        except Exception as e:
            logger.error(f"Kimi Code 请求失败: {str(e)}")
            raise

    async def generate_stream_response(
        self, messages: List[Dict]
    ) -> AsyncGenerator[str, None]:
        """生成流式响应"""
        if self.async_client is None:
            error_message = "Kimi Code 未初始化，请检查配置"
            logger.error(error_message)
            yield error_message
            return

        try:
            logger.debug(f"正在对 Kimi Code 发送流式请求: {self.model_type}")
            payload = self._build_payload(messages)

            async with self.async_client.messages.stream(**payload) as stream:
                async for event in stream:
                    if event.type == "content_block_delta":
                        if event.delta.type == "text_delta":
                            yield event.delta.text

        except Exception as e:
            logger.error(f"Kimi Code 流式请求失败: {str(e)}")
            import traceback

            traceback.print_exc()
            raise
