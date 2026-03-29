from ling_chat.core.llm_providers.base import BaseLLMProvider
from typing import Dict, List, AsyncGenerator
from openai import AsyncOpenAI
from ling_chat.core.logger import logger
import copy
import asyncio
import traceback

try:
    from memnetai import MemNetAIClient, Message
except ImportError:
    logger.error("缺少 MemNetAI SDK，请运行: pip install memnetai-python-sdk")


class MemNetAIProvider(BaseLLMProvider):
    def __init__(self, model_type: str, api_key: str, base_url: str, memnetai_api_key: str):
        super().__init__()
        self.model_type = model_type
        self.agent_name = "lingchat_player"
        self.namespace = "lingchat_main_story"

        if base_url and not base_url.endswith("/v1"):
            base_url = base_url.rstrip("/") + "/v1"

        self.llm_client = AsyncOpenAI(api_key=api_key, base_url=base_url)

        if not memnetai_api_key:
            logger.warning("未配置 MemNetAI API Key！")
            self.memnet_client = None
        else:
            self.memnet_client = MemNetAIClient(api_key=memnetai_api_key)
            logger.info(f"MemNetAI 记忆组件加载完毕！驱动模型: {model_type}")

    def initialize_client(self):
        return super().initialize_client()

    def generate_response(self, messages: List[Dict]) -> str:
        return "请使用流式接口 (Stream) 获得最佳体验。"

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        try:
            if self.llm_client is None:
                yield "【错误】大模型客户端未初始化"
                return

            last_user_msg = messages[-1].get("content", "") if messages[-1].get("role") in ["user", "human"] else ""
            memory_context = ""

            if self.memnet_client and last_user_msg:
                try:
                    logger.info(f"正在向 MemNetAI 检索关于「{last_user_msg}」的记忆...")
                    raw_result = await asyncio.to_thread(
                        self.memnet_client.recall,
                        memory_agent_name=self.agent_name,
                        namespace=self.namespace,
                        query=last_user_msg
                    )

                    memory_context = str(raw_result)
                    logger.info(f"成功拿到记忆数据: {memory_context[:100]}...")

                except Exception as e:
                    logger.warning(f"MemNetAI 回忆失败: {str(e)}")

            augmented_messages = copy.deepcopy(messages)
            if memory_context and memory_context.strip() != "":
                if augmented_messages[-1].get("role") in ["user", "human"]:
                    original_content = augmented_messages[-1]["content"]
                    augmented_messages[-1]["content"] = (
                        f"【系统提示：以下是你脑海中浮现的过往记忆】：\n{memory_context}\n\n"
                        f"【用户当前对你说】：\n{original_content}"
                    )

            full_ai_reply = ""
            logger.info("记忆检索完毕，正在等待大模型生成回复...")

            stream = await self.llm_client.chat.completions.create(
                model=self.model_type,
                messages=augmented_messages,
                stream=True
            )

            first_chunk = False
            async for chunk in stream:
                if not first_chunk:
                    logger.info("成功连通！")
                    first_chunk = True

                if chunk.choices and chunk.choices[0].delta.content:
                    content = chunk.choices[0].delta.content
                    full_ai_reply += content
                    yield content

            if self.memnet_client and last_user_msg and full_ai_reply:
                try:
                    logger.info("对话结束，正在后台静默存入新记忆...")
                    memory_messages = [
                        Message(role="user", content=last_user_msg),
                        Message(role="assistant", content=full_ai_reply)
                    ]
                    await asyncio.to_thread(
                        self.memnet_client.memories,
                        memory_agent_name=self.agent_name,
                        namespace=self.namespace,
                        messages=memory_messages,
                        async_mode=1
                    )
                except Exception as e:
                    logger.warning(f"MemNetAI 记忆写入失败: {str(e)}")

        except Exception as e:
            error_details = traceback.format_exc()
            logger.error(f"严重崩溃！详情: \n{error_details}")
            yield f"【晕】系统遇到致命错误，请看终端日志: {str(e)}"