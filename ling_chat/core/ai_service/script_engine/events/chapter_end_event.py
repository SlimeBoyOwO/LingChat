from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.core.service_manager import service_manager
from ling_chat.utils.function import Function

class ChapterEndEvent(BaseEvent):
    """处理对话事件"""
    async def execute(self):
        end_type = self.event_data.get('end_type', 'linear')
        next_chapter = self.event_data.get('next_chapter', 'end')

        return next_chapter
    

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'chapter_end'