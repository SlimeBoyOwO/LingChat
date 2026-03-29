import asyncio
import json
import os
import random
from typing import Optional

from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.message_system.message_generator import MessageGenerator
from ling_chat.core.ai_service.proactive_system.activity_monitor import UserActivityMonitor, VisualMonitor
from ling_chat.core.ai_service.proactive_system.interest_manager import InterestManager
from ling_chat.core.ai_service.proactive_system.schedule_manager import ScheduleManager
from ling_chat.core.ai_service.proactive_system.strategy_dispatcher import StrategyDispatcher
from ling_chat.core.ai_service.proactive_system.type import PerceptionResult
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.game_database.models import LineAttribute, LineBase
from ling_chat.schemas.schedule_settings import UserScheduleSettings
from ling_chat.utils.runtime_path import user_data_path


class ProactiveSystem:
    def __init__(self, config: AIServiceConfig, game_status: GameStatus, message_generator: MessageGenerator):
        self.config = config
        self.game_status = game_status
        self.enabled = os.getenv("ENABLE_PROACTIVE_SYSTEM", "false").lower() == "true"
        
        # 加载数据
        self.settings = self._load_settings()
        self.message_generator = message_generator
        
        # 初始化各子模块
        self.interest_manager = InterestManager()
        self.activity_monitor = UserActivityMonitor()
        self.visual_monitor = VisualMonitor(self.interest_manager)
        self.schedule_manager = ScheduleManager(self.settings, game_status, self.interest_manager)
        self.strategy_dispatcher = StrategyDispatcher(game_status, self.settings)
        
        # 主动对话循环任务
        self.proactive_loop_task: Optional[asyncio.Task] = None

    def start(self):
        self.enabled = os.getenv("ENABLE_PROACTIVE_SYSTEM", "false").lower() == "true"
        enable_schedule = os.getenv("ENABLE_SCHEDULE_REMINDER", "true").lower() == "true"
        enable_visual = os.getenv("ENABLE_VISUAL_PRECEPTION", "true").lower() == "true"

        # 日程提醒独立于主动对话系统，单独控制
        if enable_schedule:
            self.schedule_manager.start()
        else:
            logger.info("日程提醒已禁用")

        if not self.enabled:
            logger.info("主动对话系统已禁用")
            return

        logger.info("启动主动对话系统...")
        # 1. 启动感知线程
        self.activity_monitor.start()
        if enable_visual:
            self.visual_monitor.start()
        else:
            logger.info("视觉感知已禁用")

        # 2. 启动兴趣/随机对话主循环
        self.proactive_loop_task = asyncio.create_task(self._main_loop())

    async def reload_system(self):
        await self.cleanup()
        self.reload_schedule()
        self.interest_manager.reload_max_proactive_count()
        self.start()
    
    def reload_schedule(self):
        self.settings = self._load_settings()
        self.schedule_manager.stop()
        if os.getenv("ENABLE_SCHEDULE_REMINDER", "true").lower() == "true":
            self.schedule_manager.start()

    async def _main_loop(self):
        """
        主循环：周期性检查兴趣值，决定是否发起对话
        """
        while True:
            # 随机休眠 30s ~ 2mins
            sleep_time = random.uniform(15, 45)
            await asyncio.sleep(sleep_time)
            
            # 1. 获取感知数据
            act_status = self.activity_monitor.get_status()
            has_visual_change = self.visual_monitor.consume_change_flag()
            current_ocr = self.visual_monitor.get_current_text()
            
            perception = PerceptionResult(
                state=act_status['state'],
                description=act_status['desc'],
                interest_modifier=act_status['mod'],
                visual_change_detected=has_visual_change,
                current_screen_text=current_ocr
            )
            
            # 2. 更新兴趣值
            self.interest_manager.update(perception)
            
            # 3. 判断是否触发
            if self.interest_manager.should_trigger_talk():
                logger.info("满足主动对话条件，正在生成内容...")
                await message_broker.publish_clients(self.config.clients, (ResponseFactory.create_thinking(True)).model_dump())
                
                prompt = await self.strategy_dispatcher.get_proactive_prompt(perception)
                
                if prompt:
                    self.game_status.add_line(LineBase(attribute=LineAttribute.USER, content=prompt))
                    
                    async for response in self.message_generator.process_message_stream():
                        await message_broker.publish_clients(self.config.clients, response.model_dump())

                self.interest_manager.on_ai_reply()

    async def cleanup(self):
        """清理资源"""
        self.activity_monitor.stop()
        self.visual_monitor.stop()
        self.schedule_manager.stop()
        if self.proactive_loop_task:
            self.proactive_loop_task.cancel()
        logger.info("主动对话系统已关闭")

    def on_user_message_received(self):
        """
        外部调用接口：当用户主动发消息时调用
        用于重置兴趣衰减
        """
        self.interest_manager.on_user_reply()

    def _load_settings(self) -> UserScheduleSettings:
        schedule_data_path = user_data_path / "game_data" / "schedules.json"
        try:
            if not schedule_data_path.exists():
                return UserScheduleSettings()
            with open(schedule_data_path, 'r', encoding='utf-8') as f:
                return UserScheduleSettings(**json.load(f))
        except Exception as e:
            logger.error(f"加载日程配置失败: {e}")
            return UserScheduleSettings()