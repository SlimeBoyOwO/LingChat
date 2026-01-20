from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory


class SoundEvent(BaseEvent):
    """处理音效事件"""

    async def execute(self):
        sound:str = self.event_data.get('soundPath', '')
        duration:float = self.event_data.get('duration', '')

        logger.info(f"播放声音: {sound}")

        event_response = ResponseFactory.create_sound(sound, duration = duration)
        await message_broker.publish("1",
            event_response.model_dump()
        )

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'sound'
