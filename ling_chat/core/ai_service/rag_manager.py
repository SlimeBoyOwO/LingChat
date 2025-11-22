import os
from typing import List, Dict
from ling_chat.core.logger import logger, TermColors

class RAGManager:
    def __init__(self):
        # 兼容逻辑：只要 USE_RAG 或 USE_MEMORY_SYSTEM 为 True，就启用
        env_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        env_mem = os.environ.get("USE_MEMORY_SYSTEM", "True").lower() == "true"
        
        self.enabled = env_rag or env_mem
        
        self.memory_systems_cache = {}
        self.active_memory_system = None 
        self.character_id = 0
        
        class Config: pass
        self.memory_config = Config()
        
        if self.enabled:
            logger.info("RAGManager: 结构化记忆库系统已挂载")
        else:
            logger.info("RAGManager: 记忆系统已禁用")

    def switch_rag_system_character(self, character_id: int) -> bool:
        """切换角色的 Memory 系统"""
        self.character_id = character_id

        if not self.enabled:
            return False

        if character_id in self.memory_systems_cache:
            self.active_memory_system = self.memory_systems_cache[character_id]
            logger.info(f"Memory: 切换至角色 ID {character_id}")
            return True

        try:
            from ling_chat.core.memory import MemorySystem
            logger.info(f"Memory: 正在初始化角色 {character_id} 的记忆库...")
            new_system = MemorySystem(self.memory_config, character_id)
            
            if new_system.initialize():
                self.memory_systems_cache[character_id] = new_system
                self.active_memory_system = new_system
                return True
            return False
        except ImportError:
            logger.error("Critical: ling_chat.core.memory 模块缺失")
            return False
        except Exception as e:
            logger.error(f"Memory 初始化失败: {e}", exc_info=True)
            return False

    def rag_append_sys_message(self, current_context: List[Dict], rag_messages: List[Dict], user_input: str) -> None:
        """
        上下文组装核心逻辑 - 修正版
        保证记忆不丢失，逻辑如下：
        1. 始终保留从 `last_processed_idx` 开始的所有未归档消息。
        2. 为了上下文连贯，保留 `recent_window` 长度的"重叠区"。
        """
        if not (self.enabled and self.active_memory_system):
            return

        try:
            ms = self.active_memory_system

            system_prompt_msg = None
            history_messages = []
            
            if current_context and current_context[0]["role"] == "system":
                system_prompt_msg = current_context[0]
                history_messages = current_context[1:]
            else:
                history_messages = current_context

            ms.check_and_trigger_auto_update(history_messages)
            
            memory_text = ms.get_memory_prompt()
            memory_msg = {"role": "system", "content": memory_text}
            
            # 核心逻辑：我们要发送给 LLM 的是 [已归档记忆] + [衔接上下文] + [未归档新消息]
            # last_processed_idx 指向未归档的第一条消息
            # 计算切片起始点：
            # start_index = 已归档位置 - 回溯窗口 (例如 50 - 15 = 35)
            # 这样 LLM 能看到 35-50 (作为上下文) 以及 50-end (需要处理的新消息)
            slice_start_index = max(0, ms.last_processed_idx - ms.recent_window)
            
            sliced_history = history_messages[slice_start_index:]
            
            total_len = len(history_messages)
            kept_len = len(sliced_history)
            logger.debug(f"Memory Context: 总历史 {total_len} | 已归档至 {ms.last_processed_idx} | "
                         f"切片起始 {slice_start_index} | 实际发送 {kept_len} 条 (回溯窗口 {ms.recent_window})")

            final_context = []
            
            if system_prompt_msg:
                final_context.append(system_prompt_msg)
            
            final_context.append(memory_msg)
            
            final_context.extend(sliced_history)

            current_context.clear()
            current_context.extend(final_context)

        except Exception as e:
            logger.error(f"Memory 处理流程出错: {e}", exc_info=True)

    def force_save_memory(self):
        pass
        
    def save_messages_to_rag(self, messages):
        pass

    def prepare_messages(self, user_input):
        return []