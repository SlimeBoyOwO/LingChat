import os
from typing import List, Dict
from ling_chat.core.logger import logger

class RAGManager:
    def __init__(self):
        # 兼容原有开关，或者使用新的 MEMORY_SYSTEM_ENABLED
        self.use_memory = os.environ.get("USE_MEMORY_SYSTEM", "True").lower() == "true"
        # 如果原 RAG 开关开启，也视为开启（兼容过渡）
        if os.environ.get("USE_RAG", "False").lower() == "true":
            self.use_memory = True
            
        self.memory_systems_cache = {}  # 缓存 MemorySystem 实例
        self.memory_config = None
        self.active_memory_system = None 
        self.character_id = 0
        
        self._init_config()
        
    def _init_config(self):
        """初始化配置"""
        class Config:
            def __init__(self, **kwargs):
                for key, value in kwargs.items():
                    setattr(self, key, value)
        
        # 传递相关环境变量
        self.memory_config = Config()

        if self.use_memory:
            logger.info("正在初始化 Memory Bank 系统...")
            # 初始加载 ID 为 0 的角色 (或由外部调用 switch)
            self.switch_rag_system_character(self.character_id)
        else:
            logger.info("Memory Bank 系统已禁用")

    def switch_rag_system_character(self, character_id: int) -> bool:
        """切换或初始化指定角色的 Memory 系统"""
        self.character_id = character_id

        if not self.use_memory:
            return False

        if character_id in self.memory_systems_cache:
            self.active_memory_system = self.memory_systems_cache[character_id]
            logger.info(f"Memory Bank 已切换至角色 (ID: {character_id})")
            return True

        try:
            # 导入新的 MemorySystem
            from ling_chat.core.memory import MemorySystem
            
            new_system = MemorySystem(self.memory_config, character_id)
            
            if new_system.initialize():
                self.memory_systems_cache[character_id] = new_system
                self.active_memory_system = new_system
                return True
            return False
        except ImportError as e:
            logger.error(f"Memory模块导入失败: {e}")
            return False
        except Exception as e:
            logger.error(f"切换角色 (ID: {character_id}) Memory系统出错: {e}", exc_info=True)
            return False

    def rag_append_sys_message(self, current_context: List[Dict], rag_messages: List[Dict], user_input: str) -> None:
        """
        逻辑核心：
        1. 插入 Memory Bank 字符串到 System Prompt 之后。
        2. 执行步跳检测。
        3. 根据状态决定是保留全量历史还是截断历史。
        
        注意：rag_messages 参数在此处保留用于兼容接口，但不再用于传递检索向量。
        current_context 会被直接修改。
        """
        if not (self.use_memory and self.active_memory_system):
            return

        try:
            # 1. 步跳检测：检查当前历史长度
            # 排除 system prompt，计算实际对话轮数
            history_messages = [m for m in current_context if m["role"] != "system"]
            history_count = len(history_messages)
            
            self.active_memory_system.check_and_trigger_update(history_count)
            
            # 2. 准备 Memory Prompt
            memory_prompt_content = self.active_memory_system.get_memory_prompt()
            memory_message = {"role": "system", "content": memory_prompt_content}
            
            # 3. 定位插入点：必须在第一个 System Prompt (人设) 之后
            # 通常 current_context[0] 是人设
            insert_index = 0
            if current_context and current_context[0]["role"] == "system":
                insert_index = 1
            
            # 插入 Memory Prompt
            # 注意：为了防止重复插入，可以先检查一下
            # 这里简化逻辑，直接插入，LLM 通常能处理连续的 System Prompt
            # 或者替换掉旧的 Memory Prompt (如果之前插入过)
            
            # 4. 上下文剪裁 (Slicing)
            # 如果拥有有效的 Memory Bank 且不在更新中，我们只保留最近 N 条
            # 如果正在更新中，或者还没有生成第一次 Summary，必须保留全量历史
            
            final_context = []
            
            # 保留原始 System Prompt (人设)
            if insert_index == 1:
                final_context.append(current_context[0])
            
            # 加入 Memory Bank
            final_context.append(memory_message)
            
            # 处理历史记录
            if self.active_memory_system.has_valid_memory:
                # 只有在 Memory Bank 有效时，才截取最近 N 条
                # 读取窗口大小
                window = self.active_memory_system.recent_window
                recent_history = history_messages[-window:] if window < len(history_messages) else history_messages
                final_context.extend(recent_history)
                logger.debug(f"MemorySystem: 已截断上下文，保留 Memory Bank + 最近 {len(recent_history)} 条消息")
            else:
                # 否则发送全量历史 (等待第一次总结完成)
                final_context.extend(history_messages)
                logger.debug(f"MemorySystem: Memory Bank 尚未生成或更新中，发送全量历史 ({len(history_messages)} 条)")

            # 5. 暴力修改 current_context
            # 清空原列表并填入新列表，以影响外部引用
            current_context.clear()
            current_context.extend(final_context)

        except Exception as e:
            logger.error(f"Memory System 处理出错: {e}", exc_info=True)

    def save_messages_to_rag(self, messages):
        """
        保存消息的接口。
        在 Memory Bank 模式下，这里主要用于触发记录，
        实际的持久化通常在 trigger_background_update 异步任务中读取内存中的 messages 处理。
        保留此接口以兼容调用。
        """
        # 可以在这里做一些简单的日志记录或者追加写入到本地临时文件
        pass

    def prepare_messages(self, user_input):
        return []