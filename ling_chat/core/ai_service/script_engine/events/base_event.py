from abc import ABC, abstractmethod
from typing import Any

from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus


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
        """执行事件"""
        pass

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        """判断是否能处理指定类型的事件"""
        return False
