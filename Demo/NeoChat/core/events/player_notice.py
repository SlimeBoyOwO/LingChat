# core/events/player_notice.py
from typing import Dict, Any
from .base import BaseEventHandler
from ..logger import TermColors

class PlayerNoticeEventHandler(BaseEventHandler):
    def handle(self, params: Dict, content: str) -> bool:
        self.ui.display_system_message(f"[系统提示]: {content}", color=TermColors.BLUE)
        return True