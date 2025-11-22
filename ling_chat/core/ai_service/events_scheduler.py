import asyncio
from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.utils.function import Function
from ling_chat.core.messaging.broker import message_broker
from ling_chat.utils.runtime_path import user_data_path
from typing import List, Dict
from pathlib import Path
import re
import os

from ling_chat.core.logger import logger

class Schedule:
    def __init__(self, schedule_id: int, title: str, content: Dict, character: str = "default"):
        self.id = schedule_id
        self.character = character
        self.title = title
        self.content = content

class EventsScheduler:
    def __init__(self, config: AIServiceConfig):
        self.config = config
        self.user_name = "用户"
        self.ai_name = "AI助手"
        self.schedule_tasks: list[Schedule] = []
        
        # TODO 暂时用环境变量管理日程功能的启动，以后可以考虑更换（或者干脆别换了）
        # 检查环境变量是否启用日程功能
        self.enabled = os.getenv("ENABLE_SCHEDULE", "true").lower() == "true"
        if not self.enabled:
            logger.info("日程功能已通过环境变量禁用")
            
            return   

        schedule_data_path = user_data_path / "game_data" / "schedules"

        self.id_increaser = 0   # TODO: 使用数据库之前的暂时替代
        # self.schedule_tasks.append(Schedule(self.use_id_increaser(), deflaut_schedule_title, deflaut_schedule_tasks_content, deflaut_schedule_character))
        self.id_increaser:int = 0           # TODO: Schedule之后由数据库管理，暂时用这个代替

        self._read_schedule_data(schedule_data_path)
    
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
            if self.ai_name == schedule.character:
                user_message:str = "{时间差不多到啦，" + self.user_name + "之前拜托你提醒他:\"" + schedule.content.get(next_time, "你写的程序的日程系统有BUG，记得去修") + "\"，和" + self.user_name + "主动搭话一下吧~}"
                for client_id in self.config.clients:
                    await message_broker.enqueue_ai_message(client_id, user_message)
        
        self.proceed_next_nodification()

    async def cleanup(self):
        """简单的清理方法"""
        if hasattr(self, 'schedule_task') and self.schedule_task:
            self.schedule_task.cancel()
    
    def _read_schedule_data(self, schedule_data_path: Path):
        """
        从指定目录读取所有txt文件并转换为Schedule对象
        """
        if not schedule_data_path.exists():
            logger.info(f"日程数据目录不存在: {schedule_data_path}")
            return
        
        # 遍历目录下的所有txt文件
        for txt_file in schedule_data_path.glob("*.txt"):
            try:
                with open(txt_file, 'r', encoding='utf-8') as f:
                    content = f.read().strip()
                
                # 解析文件内容
                schedule = self._parse_schedule_file(content, txt_file)
                if schedule:
                    self.schedule_tasks.append(schedule)
                    logger.info(f"成功加载日程文件: {txt_file.name}")
                    
            except Exception as e:
                logger.error(f"读取日程文件失败 {txt_file}: {e}")
    
    def _parse_schedule_file(self, content: str, file_path: Path) -> Schedule | None:
        """
        解析单个日程文件内容
        """
        # 提取标题
        title_match = re.search(r'title\s*=\s*(.+)', content)
        title = title_match.group(1).strip() if title_match else file_path.stem
        
        # 提取角色
        character_match = re.search(r'character\s*=\s*(.+)', content)
        character = character_match.group(1).strip() if character_match else "default"
        
        # 提取内容部分
        content_match = re.search(r'content\s*=\s*"""([\s\S]*?)"""', content)
        if not content_match:
            logger.warning(f"文件 {file_path} 中未找到有效的内容部分")
            return None
        
        content_text = content_match.group(1).strip()
        schedule_content = {}
        
        # 解析每行时间安排
        for line in content_text.split('\n'):
            line = line.strip()
            if not line or ':' not in line:
                continue
                
            # 匹配时间格式和描述
            time_match = re.match(r'(\d{1,2}:\d{2})(?::\d{2})?\s*:\s*"([^"]*)"', line)
            if time_match:
                time_key = time_match.group(1)  # 只取小时和分钟
                description = time_match.group(2)
                schedule_content[time_key] = description
        
        if not schedule_content:
            logger.warning(f"文件 {file_path} 中未解析到有效的时间安排")
            return None
        
        # 创建Schedule对象
        schedule_id = self.use_id_increaser()
        return Schedule(schedule_id, title, schedule_content, character)
        
