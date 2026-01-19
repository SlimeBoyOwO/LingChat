from datetime import datetime
from typing import List, Optional
from ling_chat.core.achievement_manager import achievement_manager
from ling_chat.core.logger import logger


class AchievementTriggerHandler:
    _instance = None

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    def handle_user_message(self, message: str) -> List[dict]:
        """
        统一处理基于用户消息的成就触发逻辑
        :param message: 用户发送的消息内容
        :return: 本次触发并成功解锁的成就列表（用于前端广播）
        """
        unlocked_achievements = []
        if not message:
            return unlocked_achievements

        # 检查是否是系统/番茄钟生成的伪造消息（通常以 '{' 开头）
        is_system_message = message.strip().startswith("{")

        if is_system_message:
            for func in [self._check_first_pomodoro]:
                res = func(message)
                if res:
                    unlocked_achievements.append(res)
        else:
            # 聊天相关的成就可能一次触发多个
            for func in [self._check_first_chat, self._check_night_owl]:
                res = func(message)
                if res:
                    unlocked_achievements.append(res)

        return unlocked_achievements

    def _check_first_chat(self, message: str) -> Optional[dict]:
        """检查是否初次聊天"""
        return self._try_unlock("first_chat")

    def _check_first_pomodoro(self, message: str) -> Optional[dict]:
        """检查首次启动番茄钟成就"""
        if "我启动了番茄钟" in message:
            return self._try_unlock("first_pomodoro")
        return None

    def _check_night_owl(self, message: str) -> Optional[dict]:
        """检查是否夜猫子"""
        now = datetime.now()
        # 判断时间是否在 23:00 - 04:00 之间
        if now.hour >= 23 or now.hour < 4:
            return self._try_unlock("night_owl")
        return None


    def _try_unlock(self, achievement_id: str) -> Optional[dict]:
        """辅助方法：尝试解锁"""
        try:
            return achievement_manager.unlock(achievement_id, {})
        except Exception as e:
            logger.error(f"处理成就 {achievement_id} 触发逻辑时出错: {e}")
            return None


achievement_trigger_handler = AchievementTriggerHandler.get_instance()
