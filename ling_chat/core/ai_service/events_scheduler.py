import asyncio
from ling_chat.utils.function import Function
from ling_chat.core.messaging.broker import message_broker
from typing import List, Dict
import os

from ling_chat.core.logger import logger

class Schedule:
    def __init__(self, schedule_id: int, title: str, content: Dict):
        self.id = schedule_id
        self.title = title
        self.content = content

class EventsScheduler:
    def __init__(self, client_id: str, user_name: str, ai_name: str):
        self.client_id = client_id
        self.user_name = user_name
        self.ai_name = ai_name
        
        # TODO 暂时用环境变量管理日程功能的启动，以后可以考虑更换（或者干脆别换了）
        # 检查环境变量是否启用日程功能
        self.enabled = os.getenv("ENABLE_SCHEDULE", "true").lower() == "true"
        if not self.enabled:
            logger.info("日程功能已通过环境变量禁用")
            self.schedule_tasks: list[Schedule] = []
            return
            
        deflaut_schedule_title = "我的考研计划"
        deflaut_schedule_tasks_content = {
            "09:00": "提醒我起床哦，记得温柔一点❤",
            "10:00": "提醒我可以开始开始今天的考研英语学习了",
            "11:00": "提醒我上午的学习完成了，起来活动一下，可以准备吃午饭了",
            "12:00": "提醒我记得午休一会，下午两点要继续学习啦",
            "14:00": "提醒我可以开始学习考研数学了",
            "17:30": "提醒我该吃晚饭啦",
            "19:00": "提醒我该准备晚上学习考研408的东西啦",
            "20:00": "提醒我不要长时间久坐啦",
            "21:00": "提醒我差不多可以休息一会啦",
            "22:30": "提醒我今天的学习任务已经完毕了，可以去休息啦！",
            "00:00": "提醒我可以去更新一下我的Github项目啦",
            "02:07": "提醒我差不多可以准备去睡觉啦",
            "02:06": "提醒我睡觉之前记得刷牙哦",
        }

        self.id_increaser = 0   # TODO: 使用数据库之前的暂时替代

        self.schedule_tasks:list[Schedule] = []
        self.schedule_tasks.append(Schedule(self.use_id_increaser(), deflaut_schedule_title, deflaut_schedule_tasks_content))
        self.id_increaser:int = 0           # TODO: Schedule之后由数据库管理，暂时用这个代替
    
    def use_id_increaser(self) -> int:
        self.id_increaser += 1
        return self.id_increaser

    def get_schedule_by_id(self, schedule_id: int) -> Schedule | None:
        for schedule in self.schedule_tasks:
            if schedule.id is schedule_id:
                return schedule
        return None
    
    def remove_schedule_by_id(self, schedule_id: int) -> bool:
        schedule = self.get_schedule_by_id(schedule_id)
        if schedule is None:
            return False
        self.schedule_tasks.remove(schedule)
        return True

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
            
        # TODO: 这里的话如果有多个日程表会出BUG，暂时懒得修
        for schedule in self.schedule_tasks:
            schedule_times:list = list(schedule.content.keys())
            seconds:float = Function.calculate_time_to_next_reminder(schedule_times)
            logger.info("距离下一次提醒还有"+Function.format_seconds(seconds))
            next_time:str = Function.find_next_time(schedule_times)
            await asyncio.sleep(seconds)
            user_message:str = "{时间差不多到啦，" + self.user_name + "之前拜托你提醒他:\"" + schedule.content.get(next_time, "你写的程序的日程系统有BUG，记得去修") + "\"，和" + self.user_name + "主动搭话一下吧~}"
            await message_broker.enqueue_ai_message(self.client_id, user_message)
        
        self.proceed_next_nodification()

    async def cleanup(self):
        """简单的清理方法"""
        if hasattr(self, 'schedule_task') and self.schedule_task:
            self.schedule_task.cancel()