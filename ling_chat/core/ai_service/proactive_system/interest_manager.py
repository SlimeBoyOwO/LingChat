import os
import random

from ling_chat.core.ai_service.proactive_system.type import PerceptionResult
from ling_chat.core.logger import logger


class InterestManager:  # Reviewed
    """管理 AI 的主动对话兴趣值 (0-100)"""

    def __init__(self, max_proactive_count: int = 5):
        self.max_proactive_count = int(os.getenv("MAX_PROACTIVE_TIMES", 1))

        self.interest = 0.0
        self.max_interest_cap = 100.0
        self.initial_max_cap = 100.0
        self.status_mod = 0

        # 随主动对话次数衰减
        self.max_proactive_count = max_proactive_count
        self.decay_step = 50.0 / max_proactive_count if max_proactive_count > 0 else 0

    def reload_max_proactive_count(self):
        self.max_proactive_count = int(os.getenv("MAX_PROACTIVE_TIMES", 1))
        self.decay_step = (
            50.0 / self.max_proactive_count if self.max_proactive_count > 0 else 0
        )

    def update(self, perception: PerceptionResult):
        """周期性更新兴趣值"""
        # 1. 基础自然增长 (每分钟 5~10，这里假设调用间隔决定倍率，暂按单次调用增加)
        base_growth = random.uniform(5, 10)

        # 2. 视觉刺激奖励
        # visual_bonus = random.uniform(5, 10) if perception.visual_change_detected else 0

        # 3. 状态修正
        self.status_mod = perception.interest_modifier

        # 计算总增量
        delta = base_growth

        # 更新并钳制范围
        self.interest = max(0.0, min(self.interest + delta, self.max_interest_cap))

        logger.info(
            f"[AI主动对话兴趣] 当前: {self.interest:.1f} (封顶: {self.max_interest_cap}) | 模式: {self.status_mod}"
        )

    def add_interest(self, delta: float):
        """手动增加兴趣值"""
        self.interest = max(0.0, min(self.interest + delta, self.max_interest_cap))

    def should_trigger_talk(self) -> bool:
        """判断是否触发主动对话: (interest - 50) / 50 > random"""
        if self.interest < 50:
            return False

        probability = (self.interest + self.status_mod - 50) / 50.0
        is_triggered = probability > random.random()

        return is_triggered

    def on_ai_reply(self):
        """AI主动回复了，兴趣上限降低"""
        self.interest = 0
        self.max_interest_cap = max(0, self.max_interest_cap - self.decay_step)
        logger.info(f"主动对话触发！Interest重置。新上限: {self.max_interest_cap}")

    def on_user_reply(self):
        """用户主动回复了，兴趣上限回满"""
        self.max_interest_cap = self.initial_max_cap
        self.interest = 0
        logger.info("检测到用户回复，AI兴趣上限已恢复。")
