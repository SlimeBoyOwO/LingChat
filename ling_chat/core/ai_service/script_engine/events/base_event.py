from abc import ABC, abstractmethod
from typing import Any

from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.logger import logger


class BaseEvent(ABC):
    """事件基类"""

    def __init__(self, config: AIServiceConfig, event_data: dict[str, Any], game_status: GameStatus):
        self.config = config
        self.event_data = event_data
        self.game_status = game_status
        if self.game_status.script_status is None:
            raise ValueError("游戏剧本状态未初始化！")
        self.script_status = self.game_status.script_status
        if self.game_status.script_status.running_client_id is None:
            raise ValueError("没有记录正在运行剧本的客户端！")
        self.client_id = self.game_status.script_status.running_client_id

    @abstractmethod
    async def execute(self):
        pass

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return False

    def _get_vars_dict(self) -> dict:
        """获取剧本变量字典副本"""
        return self.script_status.vars.copy() if self.script_status.vars else {}

    def _get_var(self, name: str, default=None):
        """获取剧本变量"""
        return self.script_status.get_variable(name, default)

    def _set_var(self, name: str, value):
        """设置剧本变量"""
        self.script_status.set_variable(name, value)
        logger.debug(f"剧本变量已设置: {name} = {value}")