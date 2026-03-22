from datetime import datetime
from typing import Optional

from ling_chat.core.service_manager import service_manager
from ling_chat.core.achievement_manager import achievement_manager
from ling_chat.core.ai_service.type import AdventureConfig
from ling_chat.core.logger import logger
from ling_chat.game_database.managers.adventure_manager import AdventureManager


class AdventureTriggerSystem:
    """
    羁绊冒险解锁条件检测系统。

    在以下时机被调用：
    1. 用户发送消息后
    2. 程序启动时 / 角色切换时
    3. 冒险完成后（检查后续冒险是否解锁）
    """
    _instance: Optional["AdventureTriggerSystem"] = None

    @classmethod
    def get_instance(cls) -> "AdventureTriggerSystem":
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance

    def check_all_adventures(
        self,
        user_id: int,
        adventures: list,
        chat_count: int = 0,
    ) -> list[dict]:
        """
        检查该用户所有已锁定冒险的解锁条件。

        Args:
            user_id: 用户ID（用于全局解锁状态）
            adventures: ScriptStatus 列表（adventure.is_adventure == True 的）
            chat_count: 当前对话聊天消息数

        Returns:
            新解锁的冒险信息列表（供前端通知）
        """
        newly_unlocked = []

        for script in adventures:
            adv: AdventureConfig = script.adventure
            if not adv.is_adventure:
                continue

            # 注册冒险完成后的成就（预注册，完成时触发）
            for ach_def in adv.completion_achievements:
                ach_id = ach_def.get("id")
                if ach_id:
                    achievement_manager.register_achievement(ach_id, ach_def)

            # 查看当前解锁状态（全局）
            if AdventureManager.is_unlocked(user_id, script.folder_key):
                continue  # 已经解锁，跳过

            # 检查所有解锁条件
            if self._check_conditions(user_id, adv, chat_count):
                # 解锁！
                AdventureManager.unlock_adventure(
                    user_id=user_id,
                    adventure_folder=script.folder_key,
                    character_folder=adv.bound_character_folder,
                )

                newly_unlocked.append({
                    "adventure_folder": script.folder_key,
                    "name": script.name,
                    "description": script.description,
                    "character_folder": adv.bound_character_folder,
                    "order": adv.order,
                })

                logger.info(f"羁绊冒险已解锁: {script.name} (folder={script.folder_key})")

        return newly_unlocked

    def _check_conditions(
        self,
        user_id: int,
        adv: AdventureConfig,
        chat_count: int,
    ) -> bool:
        """检查单个冒险的所有解锁条件（AND 逻辑，全部满足才解锁）"""
        conditions = adv.unlock_conditions
        if not conditions:
            # 没有条件 = 默认解锁
            return True

        for cond in conditions:
            cond_type = cond.get("type", "")

            if cond_type == "chat_count":
                main_role = service_manager.get_ai_service().game_status.main_role
                if main_role and main_role.resource_path == adv.bound_character_folder:
                    if not self._check_chat_count(chat_count, cond.get("threshold", 0)):
                        return False

            elif cond_type == "time_range":
                if not self._check_time_range(cond.get("start_hour", 0), cond.get("end_hour", 0)):
                    return False

            elif cond_type == "adventure_completed":
                prereq = cond.get("adventure_folder", "")
                if not self._check_adventure_completed(user_id, prereq):
                    return False

            elif cond_type == "achievement_unlocked":
                ach_id = cond.get("achievement_id", "")
                if not self._check_achievement(ach_id):
                    return False

            else:
                logger.warning(f"未知的解锁条件类型: {cond_type}，跳过")

        return True

    @staticmethod
    def _check_chat_count(current_count: int, threshold: int) -> bool:
        """检查聊天消息数是否达标"""
        return current_count >= threshold

    @staticmethod
    def _check_time_range(start_hour: int, end_hour: int) -> bool:
        """
        检查当前时间是否在指定范围内。
        支持跨午夜范围（如 23:00 - 05:00）。
        """
        now_hour = datetime.now().hour
        if start_hour <= end_hour:
            return start_hour <= now_hour < end_hour
        else:
            # 跨午夜：如 23:00 - 05:00
            return now_hour >= start_hour or now_hour < end_hour

    @staticmethod
    def _check_adventure_completed(user_id: int, adventure_folder: str) -> bool:
        """检查前置冒险是否已全局完成"""
        if not adventure_folder:
            return True
        return AdventureManager.is_global_completed(user_id, adventure_folder)

    @staticmethod
    def _check_achievement(achievement_id: str) -> bool:
        """检查指定成就是否已解锁"""
        if not achievement_id:
            return True
        all_achs = achievement_manager.get_all_achievements()
        ach = all_achs.get(achievement_id, {})
        return ach.get("unlocked", False)


adventure_trigger_system = AdventureTriggerSystem.get_instance()
