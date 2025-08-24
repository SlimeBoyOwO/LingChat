# core/events/dialogue.py
from typing import Dict
from .base import BaseEventHandler
from ..llm_interface import chat_with_deepseek
from ..logger import log_error, TermColors
from ..utils.history_processor import format_history_for_llm
import config

class DialogueEventHandler(BaseEventHandler):
    def handle(self, params: Dict, content: str) -> bool:
        char_id = self.state.format_string(params['Character'])
        character = self.state.session.characters.get(char_id)
        if not character:
            log_error(f"对话事件错误: 找不到角色ID '{char_id}'")
            return True

        if params.get('Mode') == 'Preset':
            self.ui.display_dialogue(character.name, content)
            self.state.add_dialogue_history('Dialogue', character_id=char_id, content=content)
        
        elif params.get('Mode') == 'Prompt':
            messages = [{"role": "system", "content": self.state.format_string(character.prompt)}]
            
            # 使用统一的工具格式化历史记录
            history_messages = format_history_for_llm(
                self.state,
                config.LLM_CONVERSATION_HISTORY_LIMIT,
                perspective_char_id=char_id
            )
            messages.extend(history_messages)

            if content and content.strip():
                final_prompt = (
                    f"System: 根据以上对话历史，请你做出回应。你的内心想法或行动指引是：\n"
                    f"{content}\n"
                    f"请直接生成你的对话，不要带上内心独白或额外解释。"
                )
                messages.append({"role": "user", "content": final_prompt})

            response = chat_with_deepseek(messages, character.name, color_code=TermColors.CYAN)
            if response:
                self.state.add_dialogue_history('Dialogue', character_id=char_id, content=response)
            else:
                log_error("LLM未能生成响应。")
        return True