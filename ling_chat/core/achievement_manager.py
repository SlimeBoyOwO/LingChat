import json
from datetime import datetime
from typing import Dict, Optional

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import user_data_path


class AchievementManager:
    _instance = None

    # 默认成就列表，这里预定义的数据值会优先于前端试图解锁时传入的数据
    # 值可以有 title, description, type, duration, imgUrl, audioUrl
    # target_progress: 目标进度，即触发次数
    DEFAULT_ACHIEVEMENTS = {
        "first_chat": {
            "title": "初次见面",
            "description": "与钦灵完成了第一次对话",
            "type": "common",
            "target_progress": 1,
        },
        "chat_master": {
            "title": "话痨",
            "description": "与钦灵完成了 10 次对话",
            "type": "common",
            "target_progress": 10,
        },
        "first_pomodoro": {
            "title": "专注时刻",
            "description": "第一次使用番茄钟",
            "type": "common",
            "target_progress": 1,
        },
        "night_owl": {
            "title": "夜猫子",
            "description": "在深夜（23:00-04:00）与钦灵聊天",
            "type": "rare",
            "target_progress": 1,
        },
    }

    def __init__(self):
        self.achievements_file = user_data_path / "game_data" / "achievement.json"
        self.achievements_data: Dict[str, dict] = {}
        self._dirty = False
        self._load_achievements()

    def _load_achievements(self):
        """加载成就数据，如果文件不存在则初始化"""
        if self.achievements_file.exists():
            try:
                with open(self.achievements_file, "r", encoding="utf-8") as f:
                    self.achievements_data = json.load(f)
                logger.info("成就数据加载成功")
            except Exception as e:
                logger.error(f"加载成就数据失败: {e}，将使用空数据")
                self.achievements_data = {}
        else:
            # 初始化默认结构（如果文件不存在）
            self.achievements_data = {
                e: {"unlocked": False, "unlocked_at": None, "current_progress": 0}
                for e in self.DEFAULT_ACHIEVEMENTS.keys()
            }
            logger.info("成就文件不存在，正在初始化")
            self.save()

    def save(self):
        """强制保存成就数据到磁盘"""
        try:
            # 确保存储目录存在
            self.achievements_file.parent.mkdir(parents=True, exist_ok=True)

            with open(self.achievements_file, "w", encoding="utf-8") as f:
                json.dump(self.achievements_data, f, ensure_ascii=False, indent=2)

            self._dirty = False
            logger.info("成就数据已保存")
        except Exception as e:
            logger.error(f"保存成就数据失败: {e}")

    def save_if_dirty(self):
        """如果数据有变更则由于保存（用于退出时或定期调用）"""
        if self._dirty:
            logger.info("检测到未保存的成就进度，正在保存...")
            self.save()

    def increment_progress(self, achievement_id: str, amount: int = 1) -> Optional[dict]:
        """
        增加成就进度
        :param achievement_id: 成就ID
        :param amount: 增加的数量
        :return: 如果触发解锁，返回解锁的成就详情；否则返回None
        """
        achievement_def = self.DEFAULT_ACHIEVEMENTS.get(achievement_id)
        if not achievement_def:
            return None

        # 获取当前状态
        if achievement_id not in self.achievements_data:
            self.achievements_data[achievement_id] = {
                "unlocked": False,
                "unlocked_at": None,
                "current_progress": 0
            }

        # 如果已经解锁，不进行任何操作
        state = self.achievements_data[achievement_id]
        if state.get("unlocked"):
            return None

        target = achievement_def.get("target_progress", 1)

        # 增加进度
        state["current_progress"] = state.get("current_progress", 0) + amount

        # 检查是否达成目标
        if state["current_progress"] >= target:
            state["current_progress"] = target # 修正为目标值
            return self.unlock(achievement_id, {})
        else:
            # 未达成目标，只标记脏数据，不实际写入文件系统
            self._dirty = True
            return None

    def unlock(self, achievement_id: str, achievement_data: dict) -> Optional[dict]:
        """
        尝试直接解锁成就
        :param achievement_id: 成就ID
        :param achievement_data: 成就数据
        :return: 如果解锁成功，返回成就详情；如果已解锁或ID无效，返回None
        """
        # 检查该成就id是否存在定义
        achievement_def = self.DEFAULT_ACHIEVEMENTS.get(achievement_id)
        if not achievement_def:
            logger.warning(f"尝试解锁未知成就: {achievement_id}")
            return None

        # 检查是否已解锁
        current_state = self.achievements_data.get(achievement_id, {})
        if current_state.get("unlocked"):
            # 已经解锁，直接返回None
            return None

        # 执行解锁
        now_str = datetime.now().isoformat()
        target = achievement_def.get("target_progress", 1)

        self.achievements_data[achievement_id] = {
            "unlocked": True,
            "unlocked_at": now_str,
            "current_progress": target,
        }

        # 解锁是重要事件，立即保存
        self.save()

        # 返回完整的成就数据供广播，覆盖优先级：默认（预定义）数据 > 传入的数据
        return {
            **achievement_data,
            **achievement_def,
            "id": achievement_id,
        }

    def get_all_achievements(self):
        """获取所有成就状态"""
        result = {}
        for ach_id, ach_def in self.DEFAULT_ACHIEVEMENTS.items():
            state = self.achievements_data.get(
                ach_id, {"unlocked": False, "current_progress": 0}
            )
            result[ach_id] = {**ach_def, **state}
        return result

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance


achievement_manager = AchievementManager.get_instance()
