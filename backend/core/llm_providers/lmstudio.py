import requests
from typing import Dict, List
from .base import BaseLLMProvider

class LMStudioProvider(BaseLLMProvider):
    def __init__(self, config: Dict):
        self._model_type = config.get("model_type", "local-model")
        self.base_url = config.get("base_url", "http://localhost:1234/v1")
    
    def chat_completion(self, messages: List[Dict]) -> str:
        payload = {
            "model": self._model_type,
            "messages": messages,
            "temperature": 0.7
        }
        
        response = requests.post(
            f"{self.base_url}/chat/completions",
            json=payload,
            timeout=60
        )
        response.raise_for_status()
        return response.json().get("choices", [{}])[0].get("message", {}).get("content", "")
    
    @property
    def model_type(self) -> str:
        return self._model_type