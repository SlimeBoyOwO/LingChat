import asyncio
import os
import json
from pathlib import Path
from typing import Dict

from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import user_data_path

from ling_chat.schemas.schedule_settings import *

class ProactiveSystem:
    def __init__(self, config: AIServiceConfig, game_status: GameStatus):
        self.config = config
        self.schedule_tasks: list[ScheduleItem] = []
        self.user_schedule_settings: Optional[UserScheduleSettings] = None
        self.game_status = game_status

        # TODO 暂时用环境变量管理日程功能的启动，以后可以考虑更换（或者干脆别换了）
        # 检查环境变量是否启用日程功能
        self.enabled = os.getenv("ENABLE_SCHEDULE", "true").lower() == "true"
        if not self.enabled:
            logger.info("日程功能已通过环境变量禁用")

            return

        schedule_data_path = user_data_path / "game_data" / "schedules.json"

        self._read_schedule_data(schedule_data_path)

    def start_nodification_schedules(self):
        # 检查是否启用日程功能
        if not self.enabled:
            return
        self.proceed_next_nodification()
        logger.info("日程功能已经启动")

    def proceed_next_nodification(self):
        if hasattr(self, 'schedule_task') and self.schedule_task:
            self.schedule_task.cancel()
        self.schedule_task = asyncio.create_task(self.send_nodification_by_schedule())

    async def send_nodification_by_schedule(self):
        """定义好的函数，在特定时间发送提醒用户日程"""
        # 检查是否启用日程功能
        if not self.enabled:
            return

        for schedule in self.schedule_tasks:
            schedule_times:list = list(schedule.time)
            seconds:float = Function.calculate_time_to_next_reminder(schedule_times)
            logger.info("距离下一次提醒还有"+Function.format_seconds(seconds))
            next_time:str = Function.find_next_time(schedule_times)
            await asyncio.sleep(seconds)
            # if self.game_status.current_character and self.game_status.current_character.display_name == schedule.character:
            #     user_message:str = "{时间差不多到啦，" + self.game_status.player.user_name + "之前拜托你提醒他:\"" + schedule.content.get(next_time, "你写的程序的日程系统有BUG，记得去修") + "\"，和" + self.game_status.player.user_name + "主动搭话一下吧~}"
            #    await message_broker.enqueue_ai_message("global", user_message)

        for schedule_task in self.schedule_tasks:
            schedule_task.time

        self.proceed_next_nodification()

    async def cleanup(self):
        """简单的清理方法"""
        if hasattr(self, 'schedule_task') and self.schedule_task:
            self.schedule_task.cancel()

    def _read_schedule_data(self, schedule_data_path: Path):
        """从 JSON 文件中读取并解析日程数据"""
        try:
            # 检查文件是否存在
            if not schedule_data_path.exists():
                logger.warning(f"日程数据文件不存在: {schedule_data_path}")
                self.user_schedule_settings = UserScheduleSettings()
                return
            
            # 读取 JSON 文件
            with open(schedule_data_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
            
            # 使用 Pydantic 模型解析数据
            self.user_schedule_settings = UserScheduleSettings(**data)
            
            # 更新 schedule_tasks 列表
            self.schedule_tasks = []
            if self.user_schedule_settings.scheduleGroups:
                for group_name, schedule_group in self.user_schedule_settings.scheduleGroups.items():
                    for item in schedule_group.items:
                        self.schedule_tasks.append(item)
                    logger.info(f"已加载日程组: {schedule_group.title}，包含 {len(schedule_group.items)} 个日程项")
            
            # 记录待办事项组和重要日期
            if self.user_schedule_settings.todoGroups:
                logger.info(f"已加载 {len(self.user_schedule_settings.todoGroups)} 个待办事项组")
            
            if self.user_schedule_settings.importantDays:
                logger.info(f"已加载 {len(self.user_schedule_settings.importantDays)} 个重要日期")
            
            logger.info(f"总共加载了 {len(self.schedule_tasks)} 个日程项")
            
        except json.JSONDecodeError as e:
            logger.error(f"解析日程数据 JSON 文件失败: {e}")
            self.user_schedule_settings = UserScheduleSettings()
        except Exception as e:
            logger.error(f"读取日程数据时发生错误: {e}")
            self.user_schedule_settings = UserScheduleSettings()


        


