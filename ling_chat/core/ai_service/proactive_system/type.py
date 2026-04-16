from dataclasses import dataclass
from enum import Enum


class UserState(Enum):
    IDLE = "IDLE"  # 挂机
    BROWSING = "BROWSING"  # 浏览/轻度使用
    WORK = "WORK"  # 工作
    GAME = "GAME"  # 游戏


@dataclass
class PerceptionResult:
    """感知层返回的一段时间内的汇总数据"""

    state: UserState
    description: str
    interest_modifier: int  # 对AI兴趣值的修正（如工作-25，游戏+10）
    visual_change_detected: bool  # 是否检测到显著的视觉变化
    current_screen_text: str  # 当前屏幕OCR文本（用于话题生成）
