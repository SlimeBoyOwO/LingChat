from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.script_engine.events.events_handler_loader import (
    EventHandlerLoader,
)
from ling_chat.core.logger import logger


class EventsHandler:
    def __init__(self, config: AIServiceConfig, event_list: list[dict], game_status: GameStatus):
        self.progress = 0
        self.config = config
        self.game_status = game_status
        self.event_list: list[dict] = event_list
        self.current_event: dict = {}
        self._chapter_result = None  # 新增：存储章节处理结果

    def is_finished(self) -> bool:
        """判断所有事件是否处理完毕"""
        if self._chapter_result is not None:
            return True
        return self.progress >= len(self.event_list)
    
    def get_chapter_result(self) -> str:
        """获取章节处理结果（下一章节名）"""
        return self._chapter_result if self._chapter_result is not None else "end"

    async def process_next_event(self):
        """进行下一个事件的处理"""
        if self.is_finished():
            return

        self.current_event = self.event_list[self.progress]
        self.progress += 1

        # process_event现在可能返回结果
        result = await self.process_event(self.current_event)
        
        # 如果事件返回了结果（章节结束事件），保存它
        if result is not None:
            self._chapter_result = result

    async def process_event(self, event: dict):
        """处理单个事件，可能返回章节结束结果"""
        event_type = event.get('type', 'unknown')
        logger.info(f"处理事件 {self.progress}/{len(self.event_list)}: {event_type}")

        try:
            # 获取适合的事件处理器
            handler_class = EventHandlerLoader.get_handler_for_event(event)

            # 创建处理器实例并执行
            if handler_class is not None:
                handler = handler_class(self.config, event, self.game_status)
                
                # 执行处理器，并获取可能的结果
                result = await handler.execute()
                
                # 如果是章节结束事件，返回结果
                if event_type == 'chapter_end':
                    return result
                return None
            else:
                logger.error(f"找不到对应{event_type}的事件处理器，跳过当前事件")
                return None

        except Exception as e:
            logger.error(f"处理事件时出错: {event} - {e}")
            import traceback
            traceback.print_exc()
            return None
