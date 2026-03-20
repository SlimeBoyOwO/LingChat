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

        # 确保剧本变量字典存在
        if not hasattr(self.script_status, 'vars'):
            self.script_status.vars = {}

    @abstractmethod
    async def execute(self):
        """执行事件"""
        pass

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        """判断是否能处理指定类型的事件"""
        return False

    def _get_vars_dict(self) -> dict:
        """
        获取所有变量的合并字典（用于表达式求值）。
        """
        merged = {}
        # 添加全局变量
        if self.game_status.global_variables:
            merged.update(self.game_status.global_variables)
        # 添加剧本变量（剧本变量覆盖同名全局变量）
        if self.script_status and self.script_status.vars:
            merged.update(self.script_status.vars)
        return merged

    def _get_var(self, name: str, default=None):
        """
        获取指定变量的值，优先从剧本变量中查找，其次从全局变量中查找。
        如果都不存在，返回 default。
        """
        # 先从剧本变量找
        if self.script_status and self.script_status.vars and name in self.script_status.vars:
            return self.script_status.vars[name]
        # 再从全局变量找
        if name in self.game_status.global_variables:
            return self.game_status.global_variables[name]
        return default

    def _set_var(self, name: str, value):
        """
        设置变量值。
        """
        if not self.script_status:
            logger.error(f"无法存储变量 {name}，剧本状态不存在")
            return

        # 检查是否已存在于全局变量中
        if name in self.game_status.global_variables:
            self.game_status.global_variables[name] = value
            logger.debug(f"全局变量已更新: {name} = {value}")
            return

        # 否则存储到剧本变量
        if not self.script_status.vars:
            self.script_status.vars = {}
        self.script_status.vars[name] = value
        logger.debug(f"剧本变量已设置: {name} = {value}")