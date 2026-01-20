from typing import AsyncGenerator, Dict, List

from openai import OpenAI

from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class WebLLMProvider(BaseLLMProvider):
    def __init__(self, model_type: str, api_key: str, base_url: str):
        super().__init__()
        if api_key == ("" or "sk-114514"):
            error_message = "没有API_Key怎么跑啊喂！快去设置填写！"
            logger.warning(error_message)
            # 不再抛出异常，而是设置client为None表示不可用
            self.client = None
            return
        self.api_key = api_key
        self.base_url = base_url
        self.client = OpenAI(api_key=api_key, base_url=base_url)
        self.model_type = model_type
        logger.info("通用网络大模型初始化完毕！" )

    def initialize_client(self):
        return super().initialize_client()

    def generate_response(self, messages: List[Dict]) -> str:
        """生成模型响应"""
        if self.client is None:
            error_message = "通用网络大模型未初始化，请检查配置"
            logger.error(error_message)
            return error_message

        try:
            logger.debug(f"正在对通用网络大模型发送请求: {self.model_type}")
            response = self.client.chat.completions.create(
                model=self.model_type,
                messages=messages,
                stream=False
            )
            return response.choices[0].message.content

        except Exception as e:
            logger.error(f"通用网络大模型请求失败: {str(e)}")
            raise

    async def generate_stream_response(self, messages: List[Dict]) -> AsyncGenerator[str, None]:
        """
        生成流式响应
        :param messages: 消息列表
        :return: 返回一个生成器，每次迭代返回一个chunk
        """
        if self.client is None:
            error_message = "通用网络大模型未初始化，请检查配置"
            logger.error(error_message)
            yield error_message
            return

        try:
            logger.debug(f"正在对通用网络大模型发送流式请求: {self.model_type}")
            stream = self.client.chat.completions.create(
                model=self.model_type,
                messages=messages,
                stream=True
            )

            for chunk in stream:
                if chunk.choices[0].delta.content is not None:
                    yield chunk.choices[0].delta.content

        except Exception as e:
            logger.error(f"通用网络大模型{self.model_type}流式请求失败: {str(e)}")
            import traceback
            traceback.print_exc()
            raise
