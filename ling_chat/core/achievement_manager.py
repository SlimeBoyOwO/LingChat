import json
import os
from datetime import datetime
from typing import Dict, Optional

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import user_data_path


class AchievementManager:
    _instance = None

    # 默认成就列表，这里预定义的数据值会优先于前端试图解锁时传入的数据
    # 值可以有 title, description, type, duration, imgUrl, audioUrl
    DEFAULT_ACHIEVEMENTS = {
        "first_chat": {
            "title": "初次见面",
            "description": "与钦灵完成了第一次对话",
            "type": "common",
        },
        "first_pomodoro": {
            "title": "专注时刻",
            "description": "第一次完整使用番茄钟",
            "type": "common",
        },
        "night_owl": {
            "title": "夜猫子",
            "description": "在深夜（23:00-04:00）与钦灵聊天",
            "type": "rare",
        },
    }

    def __init__(self):
        self.achievements_file = user_data_path / "game_data" / "achievement.json"
        self.achievements_data: Dict[str, dict] = {}
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
                e: {"unlocked": False, "unlocked_at": None, "progress": 0.0}
                for e in self.DEFAULT_ACHIEVEMENTS.keys()
            }
            logger.info("成就文件不存在，正在初始化")
            self.save()

    def save(self):
        """保存成就数据到磁盘"""
        try:
            # 确保存储目录存在
            self.achievements_file.parent.mkdir(parents=True, exist_ok=True)

            with open(self.achievements_file, "w", encoding="utf-8") as f:
                json.dump(self.achievements_data, f, ensure_ascii=False, indent=2)
            logger.info("成就数据已保存")
        except Exception as e:
            logger.error(f"保存成就数据失败: {e}")

    def unlock(self, achievement_id: str, achievement_data: dict) -> Optional[dict]:
        """
        尝试解锁成就
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

        # 执行解锁并保存
        now_str = datetime.now().isoformat()
        self.achievements_data[achievement_id] = {
            "unlocked": True,
            "unlocked_at": now_str,
            "progress": 1.0,
        }
        self.save()

        # 返回完整的成就数据供广播，覆盖优先级：默认（预定义）数据 > 传入的数据
        return {
            **achievement_data,
            **achievement_def,
            "id": achievement_id,
        }

    def get_all_achievements(self):
        """获取所有成就状态（混合定义和状态）"""
        result = {}
        for aid, adef in self.DEFAULT_ACHIEVEMENTS.items():
            state = self.achievements_data.get(
                aid, {"unlocked": False, "progress": 0.0}
            )
            result[aid] = {**adef, **state}
        return result

    @classmethod
    def get_instance(cls):
        if cls._instance is None:
            cls._instance = cls()
        return cls._instance


achievement_manager = AchievementManager.get_instance()
