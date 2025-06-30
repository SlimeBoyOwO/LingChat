from openai import OpenAI
import os
from typing import Dict, List
from .base import BaseLLMProvider

class DeepSeekProvider(BaseLLMProvider):
    def __init__(self, config: Dict):
        self._model_type = config.get("model_type", "deepseek-chat")
        api_key = config.get("api_key") or os.getenv("CHAT_API_KEY") or os.getenv("OPENAI_API_KEY")
        base_url = config.get("base_url", "https://api.deepseek.com")
        
        if not api_key:
            raise ValueError("DeepSeek API key未配置")
            
        self.client = OpenAI(api_key=api_key, base_url=base_url)
    
    def chat_completion(self, messages: List[Dict]) -> str:
        response = self.client.chat.completions.create(
            model=self._model_type,
            messages=messages,
            stream=False
        )
        return response.choices[0].message.content
    
    @property
    def model_type(self) -> str:
        return self._model_type