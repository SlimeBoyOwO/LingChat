from ling_chat.core.llm_providers.base import BaseLLMProvider
from typing import Dict, List, AsyncGenerator
from ling_chat.core.logger import logger

try:
    from memnetai import MemNetAIClientPlus, OpenAIConfig
except ImportError:
    logger.error("缺少 MemNetAI SDK，请运行: pip install memnetai-python-sdk")


class MemNetAIProvider(BaseLLMProvider):
    def __init__(self, model_type: str, api_key: str, base_url: str, memnetai_api_key: str):
        super().__init__()

        if not memnetai_api_key:
            logger.warning("未配置 MemNetAI API Key，MemNetAI 可能无法正常工作！")

        self.model_type = model_type

        # 初始化 MemNetAI 增强版配置
        config = OpenAIConfig(
            memnetai_api_key=memnetai_api_key,
            memory_agent_name="lingchat_player",
            namespace="lingchat_main_story",
            base_url=base_url,
            api_key=api_key,
            model_name=model_type,
            temperature=0.7,
            max_tokens=2000,
            window_size=32
        )

        try:
            self.client = MemNetAIClientPlus(config)
            logger.info(f"MemNetAI 增强版客户端初始化完毕！底层模型: {model_type}")
        except Exception as e:
            logger.error(f"MemNetAI 初始化失败: {str(e)}")
            self.client = None

    def initialize_client(self):
        return super().initialize_client()

    def _get_latest_user_message(self, messages: List[Dict]) -> str:
        """从 LingChat 的消息列表中提取最新的一条用户消息"""
        for msg in reversed(messages):
            if msg.get("role") in ["user", "human"]:
                return msg.get("content", "")
        return ""

    def generate_response(self, messages: List[Dict]) -> str:
        """生成模型响应"""
        if self.client is None:
            return "MemNetAI 客户端未初始化，请检查配置"

        last_user_msg = self._get_latest_user_message(messages)
        if not last_user_msg:
            return "未检测到有效的用户输入"

        try:
            logger.debug("正在对 MemNetAI 发送请求...")
            if self.client.input(last_user_msg):
                return self.client.chat()
            else:
                return "MemNetAI 接收输入失败"

        except Exception as e:
            # 热度限流报错
            error_msg = str(e).lower()
            if "limit" in error_msg or "热度" in error_msg or "429" in error_msg:
                logger.warning(f"触发 MemNetAI 限流: {error_msg}")
                return "（哎呀，脑容量到达极限了，让我稍作休息...）"
            logger.error(f"MemNetAI 请求失败: {str(e)}")
            raise

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:

        if self.client is None:
            yield "MemNetAI 客户端未初始化，请检查配置"
            return

        last_user_msg = self._get_latest_user_message(messages)

        try:
            logger.debug("正在对 MemNetAI 发送请求...")
            if self.client.input(last_user_msg):
                response = self.client.chat()

                yield response
            else:
                yield "MemNetAI 接收输入失败"

        except Exception as e:
            # 热度限流报错
            error_msg = str(e).lower()
            if "limit" in error_msg or "热度" in error_msg or "429" in error_msg:
                logger.warning(f"触发 MemNetAI 限流: {error_msg}")
                yield "（哎呀，脑容量到达极限了，让我稍作休息...）"
                return
            logger.error(f"MemNetAI 流式请求失败: {str(e)}")
            raise

    def __del__(self):
        """释放资源"""
        if hasattr(self, 'client') and self.client:
            try:
                self.client.close()
            except:
                pass