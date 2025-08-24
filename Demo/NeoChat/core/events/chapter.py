# core/events/chapter.py
from typing import Dict, Any
from .base import BaseEventHandler

class ChapterEventHandler(BaseEventHandler):
    def handle(self, params: Dict, content: Dict) -> bool:
        self.ui.display_chapter(content.get('Title'), content.get('Description'))
        self.state.add_dialogue_history('Chapter', **content)
        return True