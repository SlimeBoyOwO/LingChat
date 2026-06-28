from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory


class AmbientEvent(BaseEvent):
    """处理环境音事件"""

    async def _execute(self):
        ambient_path = self.event_data.get("ambientPath", "")
        volume = self.event_data.get("volume", 100.0)
        loop = self.event_data.get("loop", True)
        stop = self.event_data.get("stop", False)
        fade = self.event_data.get("fade", True)

        logger.info(f"环境音事件: path={ambient_path}, stop={stop}, volume={volume}")

        event_response = ResponseFactory.create_ambient(
            ambient_path, volume=volume, loop=loop, stop=stop, fade=fade
        )
        await message_broker.publish(self.client_id, event_response.model_dump())

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == "ambient"
