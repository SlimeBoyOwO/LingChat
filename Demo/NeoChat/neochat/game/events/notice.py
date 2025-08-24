# neochat/game/events/notice.py
from typing import Dict
from .base import BaseEventHandler

# 导入新架构下的模块
from neochat.llm.client import chat_with_deepseek # LLM客户端
from neochat.platform.logging import log_error, log_info, TermColors # 日志和颜色
from neochat.memory.manager import MemoryManager # 记忆管理器
from neochat.platform.configuration import config # 配置对象

class NoticeEventHandler(BaseEventHandler):
    """
    处理公告事件，可以是预设文本或由LLM生成。通常用于DM提示。
    """
    def handle(self, params: Dict, content: str) -> bool:
        final_content = content
        if params.get('Mode') == 'Prompt':
            log_info("正在生成公告内容...")
            dm_char_id = self.state.session.story_pack_config.dm_role_id
            dm_char = self.state.session.characters.get(dm_char_id)

            dm_prompt = "你是一个剧本杀的DM（主持人）。"
            dm_name = "DM"
            if dm_char:
                dm_prompt = self.state.format_string(dm_char.prompt)
                dm_name = dm_char.name

            messages = [
                {"role": "system", "content": dm_prompt}
            ]

            # 注入对话历史，以便LLM生成相关的公告
            memory_manager = MemoryManager(self.state)
            context_messages = memory_manager.get_context_for_llm(
                history_limit=config.llm.conversation_history_limit,
                perspective_char_id=dm_char_id # 以DM视角
            )
            messages.extend(context_messages)

            # 添加生成要求
            messages.append({"role": "user", "content": f"请根据以下要求生成一条公告:\n{content}"})

            generated_content = chat_with_deepseek(messages, character_name=dm_name, color_code=TermColors.MAGENTA)
            if generated_content:
                final_content = generated_content
            else:
                log_error("公告生成失败，使用默认内容。")
                return True # 虽然失败，但游戏流程继续

        location = params.get('Location', 'popup') # 公告显示位置，默认为弹出
        self.ui.display_system_message(f"--- [{location.upper()}] 公告 ---\n{final_content}\n--------------------", color=TermColors.MAGENTA)
        self.state.add_dialogue_history('Notice', location=location, content=final_content)
        return True # 公告事件不阻塞游戏流程