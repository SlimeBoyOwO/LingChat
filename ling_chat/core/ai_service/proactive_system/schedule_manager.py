import asyncio
from datetime import datetime, timedelta
from typing import Optional

from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.proactive_system.interest_manager import InterestManager
from ling_chat.schemas.schedule_settings import ScheduleItem, UserScheduleSettings
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker

class ScheduleManager: # Reviewed
    def __init__(self, settings: UserScheduleSettings, game_status: GameStatus, interest_manager: InterestManager):
        self.settings = settings
        self.game_status = game_status
        self.interest_manager = interest_manager
        self.current_task: Optional[asyncio.Task] = None
        
    def start(self):
        # 检查是否已有运行中的任务
        if self.current_task is not None and not self.current_task.done():
            logger.info("ScheduleManager: 日程循环已经在运行中，跳过创建")
            return
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

                # 检测是否超过兴趣阈值和对话上限
                if self.interest_manager.max_interest_cap > 50:
                    # 触发提醒
                    ai_name = self.game_status.main_role.display_name if self.game_status.main_role else "你"
                    prompt = f"{{时间到了，{ai_name} 想起来{self.game_status.player.user_name}在日程里写到：{next_item.content}。提醒一下吧？}}"
                    await message_broker.enqueue_ai_message("global", prompt)
                    self.interest_manager.on_ai_reply()
                
                # 等待一小会儿避免重复触发
                await asyncio.sleep(60)
            else:
                # 没有日程或解析失败，休眠一会再检查
                await asyncio.sleep(300)