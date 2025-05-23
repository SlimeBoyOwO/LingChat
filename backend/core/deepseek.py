from openai import OpenAI
import os
import json
import copy
#from .logger import log_debug, log_info, log_error, log_text
from .new_logger import logger
from dotenv import load_dotenv

class DeepSeek:
    def __init__(self, api_key=None, base_url=None):
        load_dotenv()
           
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
        self.settings = os.environ.get("SYSTEM_PROMPT", "你是一个AI助手，请尽可能准确地回答问题。")
        self.debug_mode = os.environ.get("DEBUG_MODE", "False").lower() == "true"
        self.model_type = os.environ.get("MODEL_TYPE", "deepseek-chat")
        
        self.messages = [
            {
                "role": "system", 
                "content": self.settings
            }
        ]
        
        # RAG 系统
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_system = None
        
        logger.debug("DeepSeek LLM 服务已初始化")
        
    def init_rag_system(self, config):
        """初始化RAG系统（如果启用）"""
        if not self.use_rag:
            logger.debug("RAG系统未启用，跳过初始化")
            return False
            
        try:
            # 记录RAG初始化的详细配置
            if self.debug_mode:
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
                
                if self.debug_mode:
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

    def process_message(self, user_input):
        if user_input.lower() in ["退出", "结束"]:
            logger.info("用户请求终止程序")
            return "程序终止"
            
        self.messages.append({"role": "user", "content": user_input})
        
        # 使用RAG增强上下文
        current_context = self.messages.copy()
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

        # 若Debug模式开启，则截取发送到llm的文字信息打印到终端
        if self.debug_mode:
            logger.debug("\n------ 开发者模式：以下信息被发送给了llm ------")
            for message in current_context:
                logger.debug(f"Role: {message['role']}\nContent: {message['content']}\n")
                
            # 增加更详细的RAG信息日志
            if self.use_rag and rag_messages:
                logger.debug("\n------ RAG增强信息详情 ------")
                logger.debug(f"原始消息数: {len(self.messages)}，RAG增强后消息数: {len(current_context)}")
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
            logger.debug("正在发送请求到DeepSeek LLM...")
            response = self.client.chat.completions.create(
                model=self.model_type,
                messages=current_context,
                stream=False
            )
            ai_response = response.choices[0].message.content
            self.messages.append({"role": "assistant", "content": ai_response})
            
            # 如果启用了RAG系统，保存本次会话到RAG历史记录
            if self.use_rag and self.rag_system:
                try:
                    self.rag_system.add_session_to_history(self.messages)
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
            # 将错误响应添加到消息历史
            self.messages.append({"role": "assistant", "content": error_message})
            
            return error_message

    def load_memory(self, memory):
        """
        加载记忆存档到会话
        
        Args:
            memory: 包含对话历史的记忆存档，可以是JSON字符串或Python对象
        """
        original_messages_count = len(self.messages)
        
        if isinstance(memory, str):
            memory = json.loads(memory)  # 将JSON字符串转为Python列表
        self.messages = copy.deepcopy(memory)  # 使用深拷贝
        
        logger.info("记忆存档已经加载")
        logger.info(f"内容是：{memory}")
        logger.info(f"新的messages是：{self.messages}")
        
        # 调试信息：详细记录记忆加载前后的变化
        if self.debug_mode:
            new_messages_count = len(self.messages)
            
            # 记录消息类型统计
            role_counts = {}
            for msg in self.messages:
                role = msg.get('role', 'unknown')
                role_counts[role] = role_counts.get(role, 0) + 1
                
            role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
            
            logger.debug("\n------ 记忆加载详情 ------")
            logger.debug(f"原始消息数: {original_messages_count}, 加载后消息数: {new_messages_count}")
            logger.debug(f"消息角色分布: {role_stats}")
            logger.debug(f"------ 记忆加载结束 ------\n")
            
            # 如果启用了RAG，尝试将加载的记忆添加到RAG历史记录
            if self.use_rag and self.rag_system:
                try:
                    # 过滤掉系统提示词，只保留用户和助手的消息
                    filtered_messages = [msg for msg in self.messages if msg.get('role') in ['user', 'assistant']]
                    
                    if filtered_messages:
                        self.rag_system.add_session_to_history(filtered_messages)
                        logger.debug(f"加载的记忆已添加到RAG历史记录 (过滤后: {len(filtered_messages)}/{len(self.messages)} 条消息)")
                    else:
                        logger.debug("过滤后无历史消息可添加到RAG")
                except Exception as e:
                    logger.error(f"将加载的记忆添加到RAG历史记录时出错: {e}")

    def get_messsages(self):
        return self.messages