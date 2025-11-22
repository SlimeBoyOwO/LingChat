from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.script_engine.events.events_handler_loader import EventHandlerLoader
from ling_chat.core.ai_service.script_engine.type import GameContext
from ling_chat.core.logger import logger

class EventsHandler:
    def __init__(self, config: AIServiceConfig, event_list: list[dict], game_context: GameContext):
        self.progress = 0
        self.config = config
        self.game_context = game_context
        self.event_list: list[dict] = event_list
        self.current_event: dict = {}
    
    def is_finished(self) -> bool:
        """判断所有事件是否处理完毕"""
        return self.progress >= len(self.event_list)
    
    async def process_next_event(self):
        """进行下一个事件的处理"""
        if self.is_finished():
            return
        
        self.current_event = self.event_list[self.progress]
        self.progress += 1
        
        await self.process_event(self.current_event)
    
    async def process_event(self, event: dict):
        """处理单个事件"""
        event_type = event.get('type', 'unknown')
        logger.info(f"处理事件 {self.progress}/{len(self.event_list)}: {event_type}")
        
        try:
            # 获取适合的事件处理器
            handler_class = EventHandlerLoader.get_handler_for_event(event)
            
            # 创建处理器实例并执行
            if handler_class is not None:
                handler = handler_class( self.config,event, self.game_context)
                await handler.execute()
            else:
                logger.error(f"找不到对应{event_type}的事件处理器，跳过当前事件")
            
        except Exception as e:
            logger.error(f"处理事件时出错: {event} - {e}")
            # 可以根据需要决定是否继续执行下一个事件
