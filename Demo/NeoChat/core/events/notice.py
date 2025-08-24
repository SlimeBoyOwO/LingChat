# core/events/notice.py
from typing import Dict
from .base import BaseEventHandler
from ..llm_interface import chat_with_deepseek
from ..logger import log_error, log_info, TermColors

class NoticeEventHandler(BaseEventHandler):
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
                {"role": "system", "content": dm_prompt},
                {"role": "user", "content": f"请根据以下要求生成一条公告:\n{content}"}
            ]
            generated_content = chat_with_deepseek(messages, character_name=dm_name, color_code=TermColors.MAGENTA)
            if generated_content:
                final_content = generated_content
            else:
                log_error("公告生成失败。")
                return True

        location = params.get('Location', 'popup')
        self.ui.display_system_message(f"--- [{location.upper()}] 公告 ---\n{final_content}\n--------------------", color=TermColors.MAGENTA)
        self.state.add_dialogue_history('Notice', location=location, content=final_content)
        return True