from typing import Dict, List
from openai import OpenAI
import os
from .logger import logger
from dotenv import load_dotenv
import requests

class DeepSeek:
    def __init__(self, api_key=None, base_url=None):
        load_dotenv()
           
        # 获取LLM类型
        self.llm_provider = os.environ.get("LLM_PROVIDER", "deepseek").lower()
        logger.debug(f"LLM提供商: {self.llm_provider}")
        
        # Ollama配置
        self.use_ollama = self.llm_provider == "ollama"
        if self.use_ollama:
            self.ollama_base_url = os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434")
            self.model_type = os.environ.get("OLLAMA_MODEL", "llama3")
            logger.debug(f"Ollama 服务地址: {self.ollama_base_url}")
            logger.debug(f"Ollama 模型: {self.model_type}")
        else:
            # DeepSeek配置
            api_key = api_key or os.environ.get("CHAT_API_KEY") or os.getenv("OPENAI_API_KEY")
            base_url = base_url or os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
            if not api_key:
                logger.error("API key 未找到！请在 .env 文件中设置 CHAT_API_KEY 或 OPENAI_API_KEY")
                raise ValueError("API key 未找到！请在 .env 文件中设置 CHAT_API_KEY 或 OPENAI_API_KEY")
            logger.debug(f"API key 状态：{'已加载' if api_key else '未加载'}")
                
            self.client = OpenAI(api_key=api_key, base_url=base_url)
            self.model_type = os.environ.get("MODEL_TYPE", "deepseek-chat")
        
        logger.debug(f"{self.llm_provider.capitalize()} LLM 服务已初始化")


    def call_ollama_api(self, messages):
        """
        调用Ollama API获取回复
        """
        try:
            logger.debug(f"正在请求Ollama API: {self.ollama_base_url}/api/chat")
            
            payload = {
                "model": self.model_type,
                "messages": messages,
                "stream": False
            }
            
            response = requests.post(
                f"{self.ollama_base_url}/api/chat",
                json=payload
            )
            
            if response.status_code != 200:
                error_msg = f"Ollama API返回错误: {response.status_code} - {response.text}"
                logger.error(error_msg)
                return None, error_msg
                
            response_json = response.json()
            return response_json.get("message", {}).get("content", ""), None
            
        except Exception as e:
            error_msg = f"调用Ollama API时出错: {str(e)}"
            logger.error(error_msg)
            logger.debug(f"Ollama API错误详情:", exc_info=True)
            return None, error_msg

    def process_message(self, messages: List[Dict], user_input: str):

        try:
            # 根据不同的LLM提供商选择不同的API调用方式
            if self.use_ollama:
                logger.debug(f"正在发送请求到Ollama服务，使用模型: {self.model_type}...")
                ai_response, error = self.call_ollama_api(messages)
                if error:
                    raise Exception(error)
            else:
                logger.debug(f"正在发送请求到DeepSeek LLM，使用模型: {self.model_type}...")
                response = self.client.chat.completions.create(
                    model=self.model_type,
                    messages=messages,
                    stream=False
                )
                ai_response = response.choices[0].message.content

            return ai_response

        except Exception as e:
            logger.error(f"LLM请求失败: {str(e)}")
            logger.debug(f"API失败详情: ", exc_info=True)
            error_message = f"【生气】抱歉，我在处理您的请求时遇到了问题: {str(e)[:100]}"
            return error_message
    
    