# core/end_conditions/base.py
from abc import ABC, abstractmethod
from typing import Dict, Any, TYPE_CHECKING

# 使用 TYPE_CHECKING 来避免循环导入
if TYPE_CHECKING:
    from ..engine import GameEngine 

class BaseEndConditionHandler(ABC):
    """所有结局处理器的抽象基类。"""
    def __init__(self, engine: 'GameEngine'):
        self.engine = engine
        self.state = engine.state
        self.ui = engine.ui

    @abstractmethod
    def process(self, end_data: Dict[str, Any]):
        """处理结局条件的核心逻辑。"""
        pass