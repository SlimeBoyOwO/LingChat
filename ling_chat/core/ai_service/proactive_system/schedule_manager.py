import asyncio
from datetime import datetime, timedelta
from typing import Optional

from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.schemas.schedule_settings import ScheduleItem, UserScheduleSettings
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker

class ScheduleManager: # Reviewed
    def __init__(self, settings: UserScheduleSettings, game_status: GameStatus):
        self.settings = settings
        self.game_status = game_status
        self.current_task: Optional[asyncio.Task] = None
        
    def start(self):
        self.current_task = asyncio.create_task(self._schedule_loop())
        
    def stop(self):
        if self.current_task:
            self.current_task.cancel()

    async def _schedule_loop(self):
        """循环查找下一个最近的日程并等待"""
        while True:
            now = datetime.now()
            next_item: Optional[ScheduleItem] = None
            min_diff = float('inf')

            # 扁平化所有日程
            all_items = []
            if self.settings.scheduleGroups:
                for grp in self.settings.scheduleGroups.values():
                    all_items.extend(grp.items)
            
            # 寻找今天内最近的未来任务
            # 注意：实际场景需处理跨天逻辑，这里简化为当日时间比较
            for item in all_items:
                try:
                    # 假设 item.time 格式为 "HH:MM"
                    target_time = datetime.strptime(item.time, "%H:%M").replace(
                        year=now.year, month=now.month, day=now.day
                    )
                    
                    if target_time <= now:
                        # 如果时间已过，假设是明天的任务 (或者忽略)
                        target_time += timedelta(days=1)
                    
                    diff = (target_time - now).total_seconds()
                    if diff < min_diff:
                        min_diff = diff
                        next_item = item
                except ValueError:
                    continue

            if next_item and min_diff != float('inf'):
                logger.info(f"下一次日程提醒: {next_item.name} 于 {min_diff:.0f}秒后")
                await asyncio.sleep(min_diff)
                
                # 触发提醒
                prompt = f"{{时间到了，提醒用户：{next_item.content}。}}"
                await message_broker.enqueue_ai_message("global", prompt)
                
                # 等待一小会儿避免重复触发
                await asyncio.sleep(60)
            else:
                # 没有日程或解析失败，休眠一会再检查
                await asyncio.sleep(300)