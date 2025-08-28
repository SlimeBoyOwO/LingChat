from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger

class ShowCharacterEvent(BaseEvent):
    """处理对话事件"""
    
    async def execute(self):
        character = self.event_data.get('character', '')

        logger.info(f"角色出现:{character}")
    
    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'show_character'