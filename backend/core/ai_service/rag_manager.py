import os
from core.logger import logger

class RAGManager:
    def __init__(self):
        # RAG 系统
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_systems_cache = {}  # 缓存RAG实例 {character_id: rag_system_instance}
        self.rag_config = None       # 存储RAG配置
        self.session_file_path = None
        self.active_rag_system = None # 当前激活的RAG实例
        self.rag_window = int(os.environ.get("RAG_WINDOW_COUNT", 5)) # 短期记忆窗口大小
        self.character_id = 0
        if self.use_rag: 
            logger.info(f"当前RAG窗口大小是：{self.rag_window}")

        self._init_rag_config()
        
    def _init_rag_config(self):
        """初始化RAG相关配置并加载RAG系统"""
        class Config:
            def __init__(self, **kwargs):
                for key, value in kwargs.items():
                    setattr(self, key, value)
                    
        use_rag = self.use_rag
        ai_name = os.environ.get("AI_NAME", "钦灵")
        log_level_str = os.environ.get("LOG_LEVEL", "INFO").upper()
        print_context = os.environ.get("PRINT_CONTEXT", "False").lower() == "true"
        rag_history_path = os.environ.get("RAG_HISTORY_PATH", "rag_chat_history")
        chroma_db_path = os.environ.get("CHROMA_DB_PATH", "chroma_db_store")
        rag_retrieval_count = int(os.environ.get("RAG_RETRIEVAL_COUNT", "3"))
        rag_candidate_multiplier = int(os.environ.get("RAG_CANDIDATE_MULTIPLIER", "3"))
        rag_context_m_before = int(os.environ.get("RAG_CONTEXT_M_BEFORE", "2"))
        rag_context_n_after = int(os.environ.get("RAG_CONTEXT_N_AFTER", "2"))
        rag_prompt_prefix = os.environ.get("RAG_PROMPT_PREFIX", 
                                        "以下是根据你的问题从历史对话中检索到的相关片段，其中包含了对话发生的大致时间：")
        rag_prompt_suffix = os.environ.get("RAG_PROMPT_SUFFIX", "")
        
        rag_config = Config(
            USE_RAG=use_rag,
            AI_NAME=ai_name,
            LOG_LEVEL=log_level_str,
            PRINT_CONTEXT=print_context,
            RAG_HISTORY_PATH=rag_history_path,
            CHROMA_DB_PATH=chroma_db_path,
            RAG_RETRIEVAL_COUNT=rag_retrieval_count,
            RAG_CANDIDATE_MULTIPLIER=rag_candidate_multiplier,
            RAG_CONTEXT_M_BEFORE=rag_context_m_before,
            RAG_CONTEXT_N_AFTER=rag_context_n_after,
            RAG_PROMPT_PREFIX=rag_prompt_prefix,
            RAG_PROMPT_SUFFIX=rag_prompt_suffix
        )
        
        if use_rag:
            logger.info("正在初始化RAG系统...")
            rag_initialized = self.init_rag_system(rag_config, self.character_id)
            if rag_initialized:
                logger.info("RAG系统初始化成功")
            else:
                logger.warning("RAG系统初始化失败或禁用")
        else:
            logger.info("RAG系统已禁用")

    def init_rag_system(self, config, initial_character_id: int):
        """初始化RAG系统（如果启用）"""
        self.rag_config = config # 存储配置以备后用
        return self.switch_rag_system_character(initial_character_id)

    def switch_rag_system_character(self, character_id: int) -> bool:
        """切换或初始化指定角色的RAG系统"""
        self.character_id = character_id

        if not self.use_rag:
            return False

        # 如果已缓存，直接切换
        if character_id in self.rag_systems_cache:
            self.active_rag_system = self.rag_systems_cache[character_id]
            logger.info(f"RAG记忆库已切换至已缓存的角色 (ID: {character_id})")
            return True

        # 如果未缓存，则创建新的实例
        try:
            from core.RAG import RAGSystem
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

    # 把current_context增加RAG上下文消息
    def rag_append_sys_message(self, current_context, rag_messages, user_input) -> None:
        if not (self.use_rag and self.active_rag_system):
            return
        try:
            logger.debug("正在调用RAG系统检索相关历史信息...")
            # 清空原有内容，再 extend 新的消息
            rag_messages.clear()  
            new_messages = self.active_rag_system.prepare_rag_messages(user_input)
            rag_messages.extend(new_messages)  # 直接修改外部传入的列表
            if rag_messages:
                logger.debug(f"RAG系统返回了 {len(rag_messages)} 条上下文增强消息")
                    
                # 将RAG消息插入到系统提示后，用户消息前
                # 注意: 防止系统提示重复出现
                # 1. 找到人设提示位置
                last_system_index = 0
                        
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
                    # 计算插入位置：最后rag_window条消息之后，但至少要在第一个系统消息之后
                    insert_position = max(
                        last_system_index + 1,  # 确保在系统消息之后
                        len(current_context) - min(self.rag_window, len(current_context))  # 最后N条之后
                    )
                    
                    # 关键修改：直接操作原列表的切片赋值
                    current_context[insert_position:insert_position] = [
                        msg for msg in filtered_rag_messages 
                        if not (msg["role"] == "system" and 
                            any(sys_msg["content"] == msg["content"] 
                                for sys_msg in current_context[:last_system_index+1]))
                    ]
                else:
                    logger.debug("所有RAG消息被过滤，未向上下文添加新消息")
            else:
                logger.debug("RAG系统未返回相关历史信息")
        except Exception as e:
            logger.error(f"RAG处理过程中出错: {e}")
            logger.debug(f"RAG process error: {e}", exc_info=True)
    
    def save_messages_to_rag(self, messages):
        if self.use_rag and self.active_rag_system:
            if not self.session_file_path:
                self.session_file_path = self.active_rag_system.get_history_filepath()
            try:
                self.active_rag_system.add_session_to_history(messages, session_filepath=self.session_file_path)
                logger.debug("当前会话已保存到RAG历史记录")
            except Exception as e:
                logger.error(f"保存会话到RAG历史记录失败: {e}")
            
        logger.debug("成功获取LLM响应")

    def prepare_messages(self, user_input):
        # 准备RAG增强消息...
        return []