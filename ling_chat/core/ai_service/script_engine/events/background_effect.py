from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory


class MusicEvent(BaseEvent):
    """处理音效事件"""

    async def execute(self):
        background_effect:str = self.event_data.get('effect', '')
        duration:float = self.event_data.get('duration', '')

        logger.info(f"背景特效: {background_effect}")

        event_response = ResponseFactory.create_background_effect(background_effect, duration = duration)
        await message_broker.publish("1",
            event_response.model_dump()
        )

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'background_effect'
