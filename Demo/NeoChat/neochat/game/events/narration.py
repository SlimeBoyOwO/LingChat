# neochat/game/events/narration.py
from typing import Dict
from .base import BaseEventHandler

# 导入新架构下的模块
from neochat.llm.client import chat_with_deepseek # LLM客户端
from neochat.platform.logging import log_error, TermColors # 日志和颜色
from neochat.memory.manager import MemoryManager # 记忆管理器
from neochat.platform.configuration import config # 配置对象

class NarrationEventHandler(BaseEventHandler):
    """
    处理旁白事件，可以是预设文本或由LLM生成。
    """
    def handle(self, params: Dict, content: str) -> bool:
        if params.get('Mode') == 'Prompt':
            # LLM生成旁白模式
            narrator_prompt = "你是一个优秀的、沉浸式的故事讲述者（旁白）。请根据以下要求和对话历史，生成一段富有文采的旁白。直接输出旁白内容，不要包含任何额外解释。"
            messages = [
                {"role": "system", "content": narrator_prompt}
            ]

            # 注入对话历史，以便LLM生成相关的旁白
            memory_manager = MemoryManager(self.state)
            context_messages = memory_manager.get_context_for_llm(
                history_limit=config.llm.conversation_history_limit,
                perspective_char_id=None # 旁白是中立视角
            )
            messages.extend(context_messages)

            # 添加生成要求
            messages.append({"role": "user", "content": f"这是你的生成要求：\n{content}"})

            generated_content = chat_with_deepseek(messages, character_name="旁白", color_code=TermColors.GREY)
            if generated_content:
                self.state.add_dialogue_history('Narration', content=generated_content)
            else:
                log_error("旁白生成失败。")
        else:
            # 预设旁白模式
            self.ui.display_narration(content)
            self.state.add_dialogue_history('Narration', content=content)
        return True # 旁白事件不阻塞游戏流程