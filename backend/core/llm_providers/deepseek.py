from openai import OpenAI
from .base import BaseLLMProvider
from typing import Dict, List
from core.logger import logger
import os

class DeepSeekProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.client = None
        self.initialize_client()
    
    def initialize_client(self):
        """初始化DeepSeek客户端"""
        api_key = os.environ.get("CHAT_API_KEY") or os.getenv("OPENAI_API_KEY")
        base_url = os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
        
        if not api_key:
            raise ValueError("没有API_Key怎么跑啊喂！快去填写！")
            
        self.client = OpenAI(api_key=api_key, base_url=base_url)
        self.model_type = os.environ.get("MODEL_TYPE", "deepseek-chat")

        logger.info("通用网络大模型初始化完毕！")
    
    def generate_response(self, messages: List[Dict]) -> str:
        """生成DeepSeek模型响应"""
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