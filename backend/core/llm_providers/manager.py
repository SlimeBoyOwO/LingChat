import copy
import os
from dotenv import load_dotenv
from typing import Dict, List, Optional
import logging
from .deepseek import DeepSeekProvider
from .ollama import OllamaProvider
from .lmstudio import LMStudioProvider

logger = logging.getLogger(__name__)

class MultiModelLLM:
    PROVIDERS = {
        "deepseek": DeepSeekProvider,
        "ollama": OllamaProvider,
        "lmstudio": LMStudioProvider
    }
    
    def __init__(self, config: Optional[Dict] = None):
        load_dotenv()
        self.config = self._load_default_config()
        if config:
            self.config.update(config)
        
        self.provider = self._init_provider()
        self.rag_system = None
    
    def _load_default_config(self) -> Dict:
        return {
            "llm_provider": os.getenv("LLM_PROVIDER", "deepseek").lower(),
            "send_current_time": os.getenv("SEND_CURRENT_TIME", "False").lower() == "true",
            "use_rag": os.getenv("USE_RAG", "False").lower() == "true",
            "deepseek": {
                "api_key": os.getenv("CHAT_API_KEY") or os.getenv("OPENAI_API_KEY"),
                "base_url": os.getenv("CHAT_BASE_URL", "https://api.deepseek.com"),
                "model_type": os.getenv("DEEPSEEK_MODEL", "deepseek-chat")
            },
            "ollama": {
                "base_url": os.getenv("OLLAMA_BASE_URL", "http://localhost:11434"),
                "model_type": os.getenv("OLLAMA_MODEL", "llama3")
            },
            "lmstudio": {
                "base_url": os.getenv("LMSTUDIO_BASE_URL", "http://localhost:1234/v1"),
                "model_type": os.getenv("LMSTUDIO_MODEL", "local-model")
            }
        }
    
    def _init_provider(self):
        provider_name = self.config["llm_provider"]
        if provider_name not in self.PROVIDERS:
            raise ValueError(f"不支持的LLM提供商: {provider_name}")
        return self.PROVIDERS[provider_name](self.config.get(provider_name, {}))

    def init_rag_system(self, rag_config, initial_character_id: int) -> bool:
        """初始化RAG系统"""
        if not self.config["use_rag"]:
            logger.debug("RAG系统未启用，跳过初始化")
            return False
            
        try:
            from core.RAG import RAGSystem
            self.rag_system = RAGSystem(rag_config, initial_character_id)
            return self.rag_system.initialize()
        except ImportError as e:
            logger.error(f"无法导入RAG模块: {e}")
            return False
        except Exception as e:
            logger.error(f"RAG系统初始化失败: {e}")
            return False
    
    def process_message(self, messages: List[Dict], user_input: str) -> str:
        """处理用户输入并获取LLM响应"""
        if user_input.lower() in ["退出", "结束"]:
            return "程序终止"
            
        # 准备上下文
        context = self._prepare_context(messages, user_input)
        
        # 获取LLM响应
        response = self.provider.chat_completion(context)
        
        if response is None:
            return "【生气】抱歉，我在处理您的请求时遇到了问题"
            
        # 更新RAG系统（如果启用）
        self._update_rag_system(messages, user_input, response)
        
        return response
    
    def _prepare_context(self, messages: List[Dict], user_input: str) -> List[Dict]:
        """准备发送给LLM的上下文"""
        context = copy.deepcopy(messages)
        
        # 添加用户输入
        context.append({"role": "user", "content": user_input})
        
        # 如果启用RAG，添加相关上下文
        if self.config["use_rag"] and self.rag_system:
            rag_messages = self.rag_system.prepare_rag_messages(user_input)
            if rag_messages:
                # 将RAG消息插入到系统提示后
                last_system_idx = -1
                for i, msg in enumerate(context):
                    if msg["role"] == "system":
                        last_system_idx = i
                
                if last_system_idx >= 0:
                    context[last_system_idx+1:last_system_idx+1] = rag_messages
                    logger.debug(f"添加了{len(rag_messages)}条RAG增强消息")
        
        # 调试日志
        if logger.isEnabledFor(logging.DEBUG):
            logger.debug("\n------ 发送给LLM的上下文 ------")
            for msg in context:
                logger.debug(f"{msg['role']}: {msg['content']}")
            logger.debug("------ 上下文结束 ------\n")
        
        return context
    
    def _update_rag_system(self, messages: List[Dict], user_input: str, response: str) -> None:
        """更新RAG系统（如果启用）"""
        if not self.config["use_rag"] or not self.rag_system:
            return
            
        try:
            # 创建完整的对话记录
            dialog = messages.copy()
            dialog.extend([
                {"role": "user", "content": user_input},
                {"role": "assistant", "content": response}
            ])
            
            self.rag_system.add_session_to_history(dialog)
            logger.debug("对话已保存到RAG系统")
        except Exception as e:
            logger.error(f"更新RAG系统失败: {e}")

    @property
    def model_type(self) -> str:
        """当前使用的模型类型"""
        return self.provider.model_type
    
    @property
    def provider_name(self) -> str:
        """当前使用的提供商名称"""
        return self.config["llm_provider"]