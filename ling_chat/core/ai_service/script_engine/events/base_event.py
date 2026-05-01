from abc import ABC, abstractmethod
from typing import Any

from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction


class BaseEvent(ABC):
    """事件基类"""

    def __init__(
        self,
        config: AIServiceConfig,
        event_data: dict[str, Any],
        game_status: GameStatus,
    ):
        self.config = config
        self.event_data = event_data
        self.game_status = game_status
        if self.game_status.script_status is None:
            raise ValueError("游戏剧本状态未初始化！")
        self.script_status = self.game_status.script_status
        if self.game_status.script_status.running_client_id is None:
            raise ValueError("没有记录正在运行剧本的客户端！")
        self.client_id = self.game_status.script_status.running_client_id

    async def process(self) -> Any:
        if not self.is_executable():
            return None

        return await self._execute()

    @abstractmethod
    async def _execute(self) -> Any:
        """执行事件"""
        pass

    def is_executable(self) -> bool:
        """判断事件是否可执行"""
        condition = self.event_data.get("condition", None)
        if condition:
            # 获取剧本变量字典（只使用剧本变量，不包含全局变量）
            vars_dict = (
                self.game_status.script_status.vars.copy()
                if self.game_status.script_status
                else {}
            )
            if not ScriptFunction.evaluate(condition, vars_dict):
                return False
        return True

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        """判断是否能处理指定类型的事件"""
        return False
