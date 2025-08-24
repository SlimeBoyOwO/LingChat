# core/events/base.py
from abc import ABC, abstractmethod
from typing import Dict, Any
from ..state_manager import StateManager
from ..ui import ConsoleUI

class BaseEventHandler(ABC):
    """所有具体事件处理器的抽象基类。"""
    def __init__(self, state_manager: StateManager, ui: ConsoleUI):
        self.state = state_manager
        self.ui = ui

    @abstractmethod
    def handle(self, params: Dict, content: Any) -> bool:
        """
        处理事件的核心逻辑。
        :param params: 从事件键中解析出的参数字典 (e.g., {'Type': 'Dialogue', 'Character': 'Yuki'})
        :param content: 事件的值 (可能是字符串、字典等)
        :return: bool - True表示游戏可以继续自动执行，False表示需要暂停 (如等待玩家输入)。
        """
        pass