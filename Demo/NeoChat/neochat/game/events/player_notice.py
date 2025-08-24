# neochat/game/events/player_notice.py
from typing import Dict, Any
from .base import BaseEventHandler # 相对导入 BaseEventHandler
# 导入颜色，用于系统消息
from neochat.platform.logging import TermColors

class PlayerNoticeEventHandler(BaseEventHandler):
    """
    处理向玩家显示系统提示的事件。
    """
    def handle(self, params: Dict, content: str) -> bool:
        self.ui.display_system_message(f"[系统提示]: {content}", color=TermColors.BLUE)
        return True # 系统提示不阻塞游戏流程