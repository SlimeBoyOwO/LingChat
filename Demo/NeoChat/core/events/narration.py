# core/events/narration.py
from typing import Dict
from .base import BaseEventHandler
from ..llm_interface import chat_with_deepseek
from ..logger import log_error, TermColors

class NarrationEventHandler(BaseEventHandler):
    def handle(self, params: Dict, content: str) -> bool:
        if params.get('Mode') == 'Prompt':
            narrator_prompt = "你是一个优秀的、沉浸式的故事讲述者（旁白）。请根据以下要求和对话历史，生成一段富有文采的旁白。直接输出旁白内容，不要包含任何额外解释。"
            messages = [
                {"role": "system", "content": narrator_prompt},
                {"role": "user", "content": f"这是你的生成要求：\n{content}"}
            ]
            generated_content = chat_with_deepseek(messages, character_name="旁白", color_code=TermColors.GREY)
            if generated_content:
                self.ui.display_narration(generated_content)
                self.state.add_dialogue_history('Narration', content=generated_content)
            else:
                log_error("旁白生成失败。")
        else:
            self.ui.display_narration(content)
            self.state.add_dialogue_history('Narration', content=content)
        return True