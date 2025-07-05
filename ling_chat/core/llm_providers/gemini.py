import os
from typing import Dict, List
import google.generativeai as genai
from ling_chat.core.llm_providers.base import BaseLLMProvider
from ling_chat.core.logger import logger


class GeminiProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.client = None
        self.model_type = None
        self.initialize_client()
    
    def initialize_client(self):
        """初始化Gemini客户端"""
        api_key = os.environ.get("GEMINI_API_KEY") or os.getenv("GOOGLE_API_KEY")
        
        if not api_key:
            raise ValueError("Gemini API key is required! Please set GEMINI_API_KEY environment variable.")
            
        genai.configure(api_key=api_key)
        self.model_type = os.environ.get("GEMINI_MODEL_TYPE", "gemini-pro")
        self.client = genai.GenerativeModel(self.model_type)
        
        logger.info("Gemini provider initialized successfully!")
    
    def generate_response(self, messages: List[Dict]) -> str:
        """处理完整对话历史的版本"""
        try:
            logger.debug(f"Sending request with history to Gemini model: {self.model_type}")
            
            # 转换消息格式
            formatted_history = []
            for msg in messages:
                role = "user" if msg["role"] in ["user", "human"] else "model"
                formatted_history.append({
                    "role": role,
                    "parts": [msg["content"]]
                })
            
            chat = self.client.start_chat(history=formatted_history[:-1])
            response = chat.send_message(messages[-1]["content"])
            return response.text
            
        except Exception as e:
            logger.error(f"Gemini request with history failed: {str(e)}")
            raise