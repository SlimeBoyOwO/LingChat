# neochat/memory/manager.py
from typing import List, Dict, Optional

# 导入新模块
from neochat.game.state import StateManager
from neochat.platform.logging import log_warning, log_debug, log_rag_output
from neochat.platform.configuration import config
from . import prompts

class MemoryManager:
    """
    负责管理短期记忆（对话历史）和长期记忆（RAG）。
    为 LLM 调用准备最终的上下文。
    """
    def __init__(self, state_manager: StateManager):
        self.state = state_manager
        # self.vector_store = VectorStore(config.paths.chroma_db) # TODO: 未来集成真正的向量数据库

    def get_context_for_llm(
        self,
        history_limit: int,
        perspective_char_id: Optional[str] = None
    ) -> List[Dict[str, str]]:
        """
        为LLM准备完整的上下文，包括RAG检索的记忆和最近的对话。
        """
        # 1. 获取最近对话历史 (滑动窗口)
        recent_history = self._get_formatted_recent_history(
            limit=history_limit,
            perspective_char_id=perspective_char_id
        )

        # 2. 如果启用了RAG，获取并注入检索到的记忆
        if config.rag.enabled:
            # 使用最近的一条用户/玩家消息作为查询内容
            last_user_message = self._get_last_user_message()
            if last_user_message:
                rag_context = self._get_rag_context(query=last_user_message)
                if rag_context:
                    # 将RAG内容作为一个 system 角色的消息块注入
                    rag_block = {
                        "role": "system",
                        "content": f"{prompts.RAG_PROMPT_PREFIX}\n{rag_context}\n{prompts.RAG_PROMPT_SUFFIX}"
                    }
                    log_rag_output(f"[RAG] 注入记忆:\n{rag_context}")
                    # 将 RAG 块放在最近历史之前
                    return [rag_block] + recent_history

        return recent_history

    def _get_rag_context(self, query: str) -> Optional[str]:
        """
        【占位符】执行RAG检索。
        当前版本此功能为占位，未来将在这里实现与ChromaDB的交互。
        """
        log_debug(f"[RAG] 正在使用查询 '{query[:50]}...' 检索记忆 (当前为占位符，无实际操作)")
        # TODO: 实现真正的向量数据库查询逻辑
        # 1. self.vector_store.query(query, n_results=config.rag.retrieval_count)
        # 2. 根据查询结果，从 dialogue_history.yaml 中提取上下文 (before/after)
        # 3. 格式化成一段文本返回
        return None  # 返回 None 表示没有检索到相关记忆

    def _get_last_user_message(self) -> Optional[str]:
        """获取最后一条非AI角色的消息内容，用于RAG查询。"""
        for record in reversed(self.state.dialogue_history):
            # 检查 'Player', 'Narration', 'Notice' 等非AI对话类型
            if record.get('type') != 'Dialogue':
                 return record.get('content') or record.get('data', {}).get('content')
        return None

    def _get_formatted_recent_history(
        self,
        limit: int,
        perspective_char_id: Optional[str] = None
    ) -> List[Dict[str, str]]:
        """
        格式化最近的对话历史，以符合LLM API的格式。
        (此函数逻辑源自原 core/utils/history_processor.py)
        """
        messages = []
        session = self.state.session
        history_records = self.state.dialogue_history[-limit:]

        for record in history_records:
            record_content = record.get('content') or record.get('data', {}).get('content')
            if not record_content:
                continue

            record_type = record.get('type')

            if record_type == 'Dialogue':
                char_id = record.get('data', {}).get('character_id')
                # 从指定AI视角看，自己的发言是 'assistant'，别人是 'user'
                role = "assistant" if char_id == perspective_char_id else "user"

                # 如果不是从特定角色视角，或者发言者不是该角色，需要标明发言者
                content_prefix = ""
                if perspective_char_id is None or char_id != perspective_char_id:
                    character = session.characters.get(char_id)
                    char_name = character.name if character else "某人"
                    content_prefix = f"{char_name}: "

                messages.append({"role": role, "content": f"{content_prefix}{record_content}"})

            elif record_type == 'Player':
                player_name = session.player.name or "玩家"
                # 从任何AI视角看，玩家的发言都是 'user'
                messages.append({"role": "user", "content": f"{player_name}: {record_content}"})

            elif record_type == 'Narration':
                # 旁白作为背景信息提供给 'user'
                messages.append({"role": "user", "content": f"（旁白：{record_content}）"})

            elif record_type == 'Notice':
                # 公告也作为背景信息
                location = record.get('data', {}).get('location', 'popup')
                messages.append({"role": "user", "content": f"（[{location.upper()}] 公告：{record_content}）"})
            else:
                log_warning(f"在格式化历史记录时，跳过未处理的类型: {record_type}")

        return messages