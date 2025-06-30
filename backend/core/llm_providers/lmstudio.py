from openai import OpenAI
from .base import BaseLLMProvider
from typing import Dict, List
from core.logger import logger
import os

class LMStudioProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.client = None
        self.model_type = os.environ.get("LMSTUDIO_MODEL_TYPE", "")
        self.initialize_client()
    
    def initialize_client(self):
        """初始化LM Studio客户端"""
        base_url = os.environ.get("LMSTUDIO_BASE_URL", "http://localhost:1234/v1")
        api_key = os.environ.get("LMSTUDIO_API_KEY", "lm-studio")  # LM Studio通常不需要真实API key
        
        self.client = OpenAI(base_url=base_url, api_key=api_key)
        logger.info("LM Studio client initialized successfully")
    
    def generate_response(self, messages: List[Dict]) -> str:
        """生成LM Studio模型响应"""
        try:
            logger.debug(f"Sending request to LM Studio model")
            response = self.client.chat.completions.create(
                model=self.model_type,
                messages=messages,
                stream=False
            )
            return response.choices[0].message.content
            
        except Exception as e:
            logger.error(f"LM Studio API request failed: {str(e)}")
            raise