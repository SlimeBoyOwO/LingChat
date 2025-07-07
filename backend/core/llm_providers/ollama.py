import requests
from .base import BaseLLMProvider
from typing import Dict, List
from core.logger import logger
import os


class OllamaProvider(BaseLLMProvider):
    def __init__(self):
        super().__init__()
        self.base_url = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434")
        self.model_type = os.environ.get("OLLAMA_MODEL", "llama3")

    def initialize_client(self):
        """初始化 Ollama 客户端（占位实现）"""
        logger.info(f"Initializing Ollama client with model {self.model_type} at {self.base_url}")
        # 如果后续有 client 对象（例如 requests.Session）可以在这里初始化
        pass

    def generate_response(self, messages: List[Dict]) -> str:
        """生成Ollama模型响应"""
        try:
            logger.debug(f"Sending request to Ollama API: {self.base_url}/api/chat")

            payload = {
                "model": self.model_type,
                "messages": messages,
                "stream": False
            }

            response = requests.post(
                f"{self.base_url}/api/chat",
                json=payload
            )

            if response.status_code != 200:
                error_msg = f"Ollama API returned error: {response.status_code} - {response.text}"
                logger.error(error_msg)
                raise Exception(error_msg)

            response_json = response.json()
            return response_json.get("message", {}).get("content", "")

        except Exception as e:
            logger.error(f"Ollama API call failed: {str(e)}")
            raise
