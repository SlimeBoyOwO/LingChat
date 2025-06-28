from typing import Dict, List
from openai import OpenAI
import os
import json
import copy
from datetime import datetime
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
            if not api_key:
                try:
                    with open(".env", "r") as f:
                        for line in f:
                            if line.strip().startswith("CHAT_API_KEY"):
                                key_part = line.split("#")[0].strip()
                                if "=" in key_part:
                                    api_key = key_part.split("=", 1)[1].strip()
                                    logger.debug("从.env文件直接读取API key成功")
                                    break
                except Exception as e:
                    logger.error(f"尝试直接读取.env文件失败: {e}")
            base_url = base_url or os.environ.get("CHAT_BASE_URL", "https://api.deepseek.com")
            if not api_key:
                logger.error("API key 未找到！请在 .env 文件中设置 CHAT_API_KEY 或 OPENAI_API_KEY")
                raise ValueError("API key 未找到！请在 .env 文件中设置 CHAT_API_KEY 或 OPENAI_API_KEY")
            logger.debug(f"API key 状态：{'已加载' if api_key else '未加载'}")
                
            self.client = OpenAI(api_key=api_key, base_url=base_url)
            self.model_type = os.environ.get("MODEL_TYPE", "deepseek-chat")
        
        # 是否发送当前时间
        self.send_current_time = os.environ.get("SEND_CURRENT_TIME", "False").lower() == "true"
        logger.debug(f"发送当前时间功能状态: {'启用' if self.send_current_time else '禁用'}")
        
        # RAG 系统
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_systems_cache = {}  # 缓存RAG实例 {character_id: rag_system_instance}
        self.rag_config = None       # 存储RAG配置
        self.active_rag_system = None # 当前激活的RAG实例
        
        logger.debug(f"{self.llm_provider.capitalize()} LLM 服务已初始化")
        
    def init_rag_system(self, config, initial_character_id: int):
        """初始化RAG系统（如果启用）"""
        if not self.use_rag:
            logger.debug("RAG系统未启用，跳过初始化")
            return False
        
        self.rag_config = config # 存储配置以备后用
        return self.switch_rag_system_character(initial_character_id)

    def switch_rag_system_character(self, character_id: int) -> bool:
        """切换或初始化指定角色的RAG系统"""
        if not self.use_rag:
            return False

        # 如果已缓存，直接切换
        if character_id in self.rag_systems_cache:
            self.active_rag_system = self.rag_systems_cache[character_id]
            logger.info(f"RAG记忆库已切换至已缓存的角色 (ID: {character_id})")
            return True

        # 如果未缓存，则创建新的实例
        try:
            from .RAG import RAGSystem
            logger.info(f"正在为新角色 (ID: {character_id}) 初始化RAG记忆库...")
            
            # 记录RAG初始化的详细配置
            if logger.should_print_context():
                logger.debug("\n------ RAG初始化配置详情 ------")
                config_attrs = [attr for attr in dir(self.rag_config) if not attr.startswith('_') and not callable(getattr(self.rag_config, attr))]
                for attr in sorted(config_attrs):
                    value = getattr(self.rag_config, attr)
                    logger.debug(f"RAG配置: {attr} = {value}")
                logger.debug("------ RAG配置结束 ------\n")
            
            new_rag_system = RAGSystem(self.rag_config, character_id) # 传入character_id
            
            if new_rag_system.initialize():
                self.rag_systems_cache[character_id] = new_rag_system
                self.active_rag_system = new_rag_system
                logger.info(f"角色 (ID: {character_id}) 的RAG记忆库初始化成功并已缓存。")
                
                if logger.should_print_context():
                    # 记录初始化后的状态信息
                    history_count = 0
                    chroma_count = 0
                    if hasattr(new_rag_system, 'flat_historical_messages'):
                        history_count = len(new_rag_system.flat_historical_messages)
                    if new_rag_system.chroma_collection:
                        chroma_count = new_rag_system.chroma_collection.count()
                    
                    logger.debug(f"RAG初始化状态: 历史消息数={history_count}, ChromaDB条目数={chroma_count}")
                
                return True
            else:
                logger.error(f"为角色 (ID: {character_id}) 初始化RAG记忆库失败。")
                return False
        except ImportError as e:
            logger.error(f"RAG模块: {e}")
            return False
        except Exception as e:
            logger.error(f"切换RAG角色 (ID: {character_id}) 时出错: {e}", exc_info=True)
            return False

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
        if user_input.lower() in ["退出", "结束"]:
            logger.info("用户请求终止程序")
            return "程序终止"
            
        # messages.append({"role": "user", "content": user_input})
        
        # 使用RAG增强上下文
        current_context = messages.copy()
        rag_messages = []
        
        if self.use_rag and self.active_rag_system:
            try:
                logger.debug("正在调用RAG系统检索相关历史信息...")
                rag_messages = self.active_rag_system.prepare_rag_messages(user_input)
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
                
                # 输出模型名称和其他参数
                logger.debug(f"使用模型: {self.model_type}")
                
                # 分析RAG消息类型统计
                role_counts = {}
                for msg in rag_messages:
                    role = msg.get('role', 'unknown')
                    role_counts[role] = role_counts.get(role, 0) + 1
                
                role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
                logger.debug(f"RAG消息角色分布: {role_stats}")
                
            logger.debug("------ 结束 ------")

        try:
            # 根据不同的LLM提供商选择不同的API调用方式
            if self.use_ollama:
                logger.debug(f"正在发送请求到Ollama服务，使用模型: {self.model_type}...")
                ai_response, error = self.call_ollama_api(current_context)
                if error:
                    raise Exception(error)
            else:
                logger.debug(f"正在发送请求到DeepSeek LLM，使用模型: {self.model_type}...")
                response = self.client.chat.completions.create(
                    model=self.model_type,
                    messages=current_context,
                    stream=False
                )
                ai_response = response.choices[0].message.content
            
            # TODO 在传入给ai之前，最好先检验一下这个对话是否是健全的
            # messages.append({"role": "assistant", "content": ai_response})
            
            # 如果启用了RAG系统，保存本次会话到RAG历史记录
            if self.use_rag and self.active_rag_system:
                try:
                    self.active_rag_system.add_session_to_history(messages)
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

    # 暂未调用该段代码↓        
    def load_memory_to_rag(self, messages):
        # 如果启用了RAG，尝试将加载的记忆添加到RAG历史记录
        if self.use_rag and self.active_rag_system:
            try:
                # 过滤掉系统提示词，只保留用户和助手的消息
                filtered_messages = [msg for msg in messages if msg.get('role') in ['user', 'assistant']]
                    
                if filtered_messages:
                    self.active_rag_system.add_session_to_history(filtered_messages)
                    logger.debug(f"加载的记忆已添加到RAG历史记录 (过滤后: {len(filtered_messages)}/{len(messages)} 条消息)")
                else:
                    logger.debug("过滤后无历史消息可添加到RAG")
            except Exception as e:
                    logger.error(f"将加载的记忆添加到RAG历史记录时出错: {e}")