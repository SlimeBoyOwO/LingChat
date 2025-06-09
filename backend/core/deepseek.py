from typing import Dict, List, Any, Optional
from openai import OpenAI
import os
import json
import copy
from datetime import datetime
#from .logger import log_debug, log_info, log_error, log_text
from .logger import logger
from dotenv import load_dotenv
import requests

# =====================
# 抽象基类
# =====================
class BaseLLMClient:
    """
    所有大模型API客户端的抽象基类，定义统一接口，便于多模型适配和扩展。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        self.api_key = api_key
        self.base_url = base_url
        self.model_type = model_type

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        统一聊天接口，所有子类需实现。
        :param messages: 标准OpenAI格式消息（支持多模态扩展）
        :return: 回复内容字符串
        """
        raise NotImplementedError

    def supports_multimodal(self) -> bool:
        """
        是否支持多模态（图片/音频等）。如支持，子类需重写。
        """
        return False

# =====================
# DeepSeek 官方API
# =====================
class DeepSeekClient(BaseLLMClient):
    """
    DeepSeek 官方API客户端，兼容OpenAI格式。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        super().__init__(api_key, base_url or "https://api.deepseek.com", model_type or "deepseek-chat")
        self.client = OpenAI(api_key=self.api_key, base_url=self.base_url)

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        调用DeepSeek官方API获取回复。
        """
        response = self.client.chat.completions.create(
            model=self.model_type,
            messages=messages,
            stream=False
        )
        return response.choices[0].message.content

# =====================
# Qwen 官方API
# =====================
class QwenClient(BaseLLMClient):
    """
    Qwen（通义千问）官方API客户端。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        super().__init__(api_key, base_url or "https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation", model_type or "qwen-turbo")

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        调用Qwen官方API获取回复。
        """
        headers = {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json"
        }
        payload = {
            "model": self.model_type,
            "input": {"messages": messages},
            "parameters": {"result_format": "message"}
        }
        resp = requests.post(self.base_url, headers=headers, json=payload)
        resp.raise_for_status()
        return resp.json()["output"]["choices"][0]["message"]["content"]

# =====================
# OpenAI GPT 官方API
# =====================
class GPTClient(BaseLLMClient):
    """
    OpenAI GPT 官方API客户端。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        super().__init__(api_key, base_url or "https://api.openai.com/v1", model_type or "gpt-3.5-turbo")
        self.client = OpenAI(api_key=self.api_key, base_url=self.base_url)

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        调用OpenAI官方API获取回复。
        """
        response = self.client.chat.completions.create(
            model=self.model_type,
            messages=messages,
            stream=False
        )
        return response.choices[0].message.content

# =====================
# Claude 官方API
# =====================
class ClaudeClient(BaseLLMClient):
    """
    Claude 官方API客户端。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        super().__init__(api_key, base_url or "https://api.anthropic.com/v1/messages", model_type or "claude-3-opus-20240229")

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        调用Claude官方API获取回复。需将OpenAI格式转换为Anthropic格式。
        """
        headers = {
            "x-api-key": self.api_key,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json"
        }
        # Claude消息格式转换
        claude_msgs = []
        for msg in messages:
            if msg["role"] == "user":
                claude_msgs.append({"role": "user", "content": msg["content"]})
            elif msg["role"] == "assistant":
                claude_msgs.append({"role": "assistant", "content": msg["content"]})
            elif msg["role"] == "system":
                # Claude不直接支持system，可拼接到user前
                if claude_msgs and claude_msgs[0]["role"] == "user":
                    claude_msgs[0]["content"] = msg["content"] + "\n" + claude_msgs[0]["content"]
                else:
                    claude_msgs.insert(0, {"role": "user", "content": msg["content"]})
        payload = {
            "model": self.model_type,
            "max_tokens": 2048,
            "messages": claude_msgs
        }
        resp = requests.post(self.base_url, headers=headers, json=payload)
        resp.raise_for_status()
        return resp.json()["content"][0]["text"]

# =====================
# Gemini 官方API
# =====================
class GeminiClient(BaseLLMClient):
    """
    Gemini 官方API客户端。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        super().__init__(api_key, base_url or "https://generativelanguage.googleapis.com/v1beta/models", model_type or "gemini-pro")

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        调用Gemini官方API获取回复。需将OpenAI格式转换为Gemini格式。
        """
        url = f"{self.base_url}/{self.model_type}:generateContent?key={self.api_key}"
        # Gemini消息格式转换
        gemini_msgs = []
        for msg in messages:
            if msg["role"] == "user":
                gemini_msgs.append({"parts": [{"text": msg["content"]}], "role": "user"})
            elif msg["role"] == "assistant":
                gemini_msgs.append({"parts": [{"text": msg["content"]}], "role": "model"})
        payload = {"contents": gemini_msgs}
        resp = requests.post(url, json=payload)
        resp.raise_for_status()
        return resp.json()["candidates"][0]["content"]["parts"][0]["text"]

# =====================
# Ollama 本地API
# =====================
class OllamaClient(BaseLLMClient):
    """
    Ollama 本地部署API客户端。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        super().__init__(api_key, base_url or "http://localhost:11434", model_type or "llama3")

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        调用Ollama本地API获取回复。
        """
        payload = {
            "model": self.model_type,
            "messages": messages,
            "stream": False
        }
        resp = requests.post(f"{self.base_url}/api/chat", json=payload)
        resp.raise_for_status()
        return resp.json().get("message", {}).get("content", "")

# =====================
# LM Studio 本地API
# =====================
class LMStudioClient(BaseLLMClient):
    """
    LM Studio 本地部署API客户端。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None):
        super().__init__(api_key, base_url or "http://localhost:1234/v1", model_type or "lmstudio-model")

    def chat(self, messages: List[Dict], **kwargs) -> str:
        """
        调用LM Studio本地API获取回复。
        """
        payload = {
            "model": self.model_type,
            "messages": messages,
            "stream": False
        }
        resp = requests.post(f"{self.base_url}/chat/completions", json=payload)
        resp.raise_for_status()
        return resp.json()["choices"][0]["message"]["content"]

# =====================
# 统一入口服务
# =====================
class LLMService:
    """
    统一大模型服务入口，根据配置选择不同的LLM客户端，支持RAG和多模态扩展。
    """
    def __init__(self, api_key=None, base_url=None, model_type=None, provider=None):
        load_dotenv()
        self.provider = (provider or os.environ.get("LLM_PROVIDER", "deepseek")).lower()
        # 根据provider自动获取对应的key、base_url、model_type
        if self.provider == "deepseek":
            self.api_key = api_key or os.environ.get("CHAT_API_KEY")
            self.base_url = base_url or os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
            self.model_type = model_type or os.environ.get("MODEL_TYPE", "deepseek-chat")
        elif self.provider == "qwen":
            self.api_key = api_key or os.environ.get("QWEN_API_KEY")
            self.base_url = base_url or os.environ.get("QWEN_BASE_URL", "https://dashscope.aliyuncs.com/api/v1/services/aigc/text-generation/generation")
            self.model_type = model_type or os.environ.get("QWEN_MODEL_TYPE", "qwen-turbo")
        elif self.provider == "gpt":
            self.api_key = api_key or os.environ.get("OPENAI_API_KEY")
            self.base_url = base_url or os.environ.get("OPENAI_BASE_URL", "https://api.openai.com/v1")
            self.model_type = model_type or os.environ.get("OPENAI_MODEL_TYPE", "gpt-3.5-turbo")
        elif self.provider == "claude":
            self.api_key = api_key or os.environ.get("CLAUDE_API_KEY")
            self.base_url = base_url or os.environ.get("CLAUDE_BASE_URL", "https://api.anthropic.com/v1/messages")
            self.model_type = model_type or os.environ.get("CLAUDE_MODEL_TYPE", "claude-3-opus-20240229")
        elif self.provider == "gemini":
            self.api_key = api_key or os.environ.get("GEMINI_API_KEY")
            self.base_url = base_url or os.environ.get("GEMINI_BASE_URL", "https://generativelanguage.googleapis.com/v1beta/models")
            self.model_type = model_type or os.environ.get("GEMINI_MODEL_TYPE", "gemini-pro")
        elif self.provider == "ollama":
            self.api_key = api_key  # 本地部署通常不需要key
            self.base_url = base_url or os.environ.get("OLLAMA_BASE_URL", "http://localhost:11434")
            self.model_type = model_type or os.environ.get("OLLAMA_MODEL", "llama3")
        elif self.provider == "lmstudio":
            self.api_key = api_key  # 本地部署通常不需要key
            self.base_url = base_url or os.environ.get("LMSTUDIO_BASE_URL", "http://localhost:1234/v1")
            self.model_type = model_type or os.environ.get("LMSTUDIO_MODEL", "lmstudio-model")
        else:
            raise ValueError(f"未知的LLM_PROVIDER: {self.provider}")
        self.client = self._init_client()
        self.send_current_time = os.environ.get("SEND_CURRENT_TIME", "False").lower() == "true"
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_system = None
        logger.debug(f"LLMService初始化，provider={self.provider}")

    def _init_client(self):
        """
        根据provider选择对应的LLM客户端。
        """
        if self.provider == "deepseek":
            return DeepSeekClient(self.api_key, self.base_url, self.model_type)
        elif self.provider == "qwen":
            return QwenClient(self.api_key, self.base_url, self.model_type)
        elif self.provider == "gpt":
            return GPTClient(self.api_key, self.base_url, self.model_type)
        elif self.provider == "claude":
            return ClaudeClient(self.api_key, self.base_url, self.model_type)
        elif self.provider == "gemini":
            return GeminiClient(self.api_key, self.base_url, self.model_type)
        elif self.provider == "ollama":
            return OllamaClient(self.api_key, self.base_url, self.model_type)
        elif self.provider == "lmstudio":
            return LMStudioClient(self.api_key, self.base_url, self.model_type)
        else:
            raise ValueError(f"未知的LLM_PROVIDER: {self.provider}")

    def init_rag_system(self, config):
        """
        初始化RAG系统（如启用）。
        """
        if not self.use_rag:
            logger.debug("RAG系统未启用，跳过初始化")
            return False
            
        try:
            # 记录RAG初始化的详细配置
            if logger.should_print_context():
                logger.debug("\n------ RAG初始化配置详情 ------")
                config_attrs = [attr for attr in dir(config) if not attr.startswith('_') and not callable(getattr(config, attr))]
                for attr in sorted(config_attrs):
                    value = getattr(config, attr)
                    logger.debug(f"RAG配置: {attr} = {value}")
                logger.debug("------ RAG配置结束 ------\n")
                
            # 动态导入，避免在未启用RAG时也必须安装相关依赖
            from .RAG import RAGSystem
            self.rag_system = RAGSystem(config)
            rag_initialized = self.rag_system.initialize()
            if rag_initialized:
                logger.info("RAG系统初始化成功")
                
                if logger.should_print_context():
                    # 记录初始化后的状态信息
                    history_count = 0
                    chroma_count = 0
                    if hasattr(self.rag_system, 'flat_historical_messages'):
                        history_count = len(self.rag_system.flat_historical_messages)
                    if self.rag_system.chroma_collection:
                        chroma_count = self.rag_system.chroma_collection.count()
                    
                    logger.debug(f"RAG初始化状态: 历史消息数={history_count}, ChromaDB条目数={chroma_count}")
            else:
                logger.info("RAG系统初始化失败或被禁用")
            return rag_initialized
        except ImportError as e:
            logger.error(f"RAG模块: {e}")
            return False
        except Exception as e:
            logger.error(f"初始化RAG系统时出错: {e}")
            return False

    def process_message(self, messages: List[Dict], user_input: str):
        """
        处理用户输入，自动调用RAG和LLM，返回回复内容。
        """
        if user_input.lower() in ["退出", "结束"]:
            logger.info("用户请求终止程序")
            return "程序终止"
            
        messages.append({"role": "user", "content": user_input})
        
        # 使用RAG增强上下文
        current_context = messages.copy()
        rag_messages = []
        
        if self.use_rag and self.rag_system:
            try:
                logger.debug("正在调用RAG系统检索相关历史信息...")
                rag_messages = self.rag_system.prepare_rag_messages(user_input)
                if rag_messages:
                    logger.debug(f"RAG系统返回了 {len(rag_messages)} 条上下文增强消息")
                    
                    # 将RAG消息插入到系统提示后，用户消息前
                    # 注意: 防止系统提示重复出现
                    # 1. 找到最后一个系统提示位置
                    last_system_index = -1
                    for i, msg in enumerate(current_context):
                        if msg["role"] == "system":
                            last_system_index = i
                            
                    # 2. 过滤RAG消息中的系统提示词，避免重复
                    filtered_rag_messages = []
                    for msg in rag_messages:
                        # 只有当RAG消息是前缀/后缀提示，且不与原系统提示重复时才添加
                        if msg["role"] == "system":
                            is_duplicate = False
                            # 检查是否与原系统提示重复
                            for sys_msg in current_context[:last_system_index+1]:
                                if sys_msg["role"] == "system" and sys_msg["content"] == msg["content"]:
                                    is_duplicate = True
                                    break
                            if not is_duplicate:
                                filtered_rag_messages.append(msg)
                        else:
                            # 非系统消息直接添加
                            filtered_rag_messages.append(msg)
                    
                    if filtered_rag_messages:
                        # 在最后一个系统消息后插入RAG消息
                        current_context = current_context[:last_system_index+1] + filtered_rag_messages + current_context[last_system_index+1:]
                        logger.debug(f"添加了 {len(filtered_rag_messages)} 条RAG消息 (过滤前: {len(rag_messages)})")
                    else:
                        logger.debug("所有RAG消息被过滤，未向上下文添加新消息")
                else:
                    logger.debug("RAG系统未返回相关历史信息")
            except Exception as e:
                logger.error(f"RAG处理过程中出错: {e}")
                logger.debug(f"RAG process error: {e}", exc_info=True)

        # 若打印上下文选项开启且在DEBUG级别，则截取发送到llm的文字信息打印到终端
        if logger.should_print_context():
            logger.debug("\n------ 开发者模式：以下信息被发送给了llm ------")
            for message in current_context:
                logger.debug(f"Role: {message['role']}\nContent: {message['content']}\n")
                
            # 增加更详细的RAG信息日志
            if self.use_rag and rag_messages:
                logger.debug("\n------ RAG增强信息详情 ------")
                logger.debug(f"原始消息数: {len(messages)}，RAG增强后消息数: {len(current_context)}")
                logger.debug(f"RAG增强消息数量: {len(rag_messages)}，位置: 系统提示后、用户消息前")
                
                # 计算并输出RAG消息的总长度（字符数）
                total_rag_chars = sum(len(msg.get('content', '')) for msg in rag_messages)
                logger.debug(f"RAG增强内容总长度: {total_rag_chars} 字符")
                logger.debug(f"使用模型: {self.client.model_type}")
                role_counts = {}
                for msg in rag_messages:
                    role = msg.get('role', 'unknown')
                    role_counts[role] = role_counts.get(role, 0) + 1
                
                role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
                logger.debug(f"RAG消息角色分布: {role_stats}")
                
            logger.debug("------ 结束 ------")

        try:
            ai_response = self.client.chat(current_context)
            messages.append({"role": "assistant", "content": ai_response})
            
            # 如果启用了RAG系统，保存本次会话到RAG历史记录
            if self.use_rag and self.rag_system:
                try:
                    self.rag_system.add_session_to_history(messages)
                    logger.debug("当前会话已保存到RAG历史记录")
                except Exception as e:
                    logger.error(f"保存会话到RAG历史记录失败: {e}")
            
            logger.debug("成功获取LLM响应")

            return ai_response

        except Exception as e:
            logger.error(f"LLM请求失败: {str(e)}")
            logger.debug(f"API失败详情: ", exc_info=True)
            
            # 创建一个有意义的错误响应，而不只是"ERROR"
            error_message = f"【生气】抱歉，我在处理您的请求时遇到了问题: {str(e)[:100]}"
            
            return error_message

    def load_memory(self, messages, memory):
        """
        加载记忆存档到会话
        
        Args:
            memory: 包含对话历史的记忆存档，可以是JSON字符串或Python对象
        """
        original_messages_count = len(messages)
        
        if isinstance(memory, str):
            memory = json.loads(memory)  # 将JSON字符串转为Python列表
        messages = copy.deepcopy(memory)  # 使用深拷贝
        
        logger.info("记忆存档已经加载")
        logger.info(f"内容是：{memory}")
        logger.info(f"新的messages是：{messages}")
        
        # 调试信息：详细记录记忆加载前后的变化
        if logger.should_print_context():
            new_messages_count = len(messages)
            
            # 记录消息类型统计
            role_counts = {}
            for msg in messages:
                role = msg.get('role', 'unknown')
                role_counts[role] = role_counts.get(role, 0) + 1
                
            role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
            
            logger.debug("\n------ 记忆加载详情 ------")
            logger.debug(f"原始消息数: {original_messages_count}, 加载后消息数: {new_messages_count}")
            logger.debug(f"消息角色分布: {role_stats}")
            logger.debug(f"------ 记忆加载结束 ------\n")

    # 暂未调用该段代码↓        
    def load_memory_to_rag(self, messages):
        # 如果启用了RAG，尝试将加载的记忆添加到RAG历史记录
        if self.use_rag and self.rag_system:
            try:
                # 过滤掉系统提示词，只保留用户和助手的消息
                filtered_messages = [msg for msg in messages if msg.get('role') in ['user', 'assistant']]
                    
                if filtered_messages:
                    self.rag_system.add_session_to_history(filtered_messages)
                    logger.debug(f"加载的记忆已添加到RAG历史记录 (过滤后: {len(filtered_messages)}/{len(messages)} 条消息)")
                else:
                    logger.debug("过滤后无历史消息可添加到RAG")
            except Exception as e:
                logger.error(f"将加载的记忆添加到RAG历史记录时出错: {e}")

DeepSeek = LLMService