import requests
from typing import Dict, List
from .base import BaseLLMProvider

class OllamaProvider(BaseLLMProvider):
    def __init__(self, config: Dict):
        self._model_type = config.get("model_type", "llama3")
        self.base_url = config.get("base_url", "http://localhost:11434")
    
    def chat_completion(self, messages: List[Dict]) -> str:
        payload = {
            "model": self._model_type,
            "messages": messages,
            "stream": False
        }
        
        response = requests.post(
            f"{self.base_url}/api/chat",
            json=payload,
            timeout=60
        )
        response.raise_for_status()
        return response.json().get("message", {}).get("content", "")
    
    @property
    def model_type(self) -> str:
        return self._model_type