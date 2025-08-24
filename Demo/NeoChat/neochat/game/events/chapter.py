# neochat/game/events/chapter.py
from typing import Dict, Any
from .base import BaseEventHandler # 相对导入 BaseEventHandler

class ChapterEventHandler(BaseEventHandler):
    """
    处理章节开始事件，用于显示章节标题和描述。
    """
    def handle(self, params: Dict, content: Dict) -> bool:
        # 显示章节信息
        self.ui.display_chapter(content.get('Title'), content.get('Description'))
        # 将章节信息添加到对话历史，方便回顾或作为LLM上下文
        self.state.add_dialogue_history('Chapter', **content)
        return True # 章节事件不阻塞游戏流程