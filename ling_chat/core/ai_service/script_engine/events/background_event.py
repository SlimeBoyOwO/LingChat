from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory


class BackgroundEvent(BaseEvent):
    """处理背景切换事件"""

    async def execute(self):
        image = self.event_data.get('imagePath', '')
        duration = self.event_data.get('duration', 1.0)

        logger.info(f"切换背景: {image}")

        # 更新游戏状态
        self.game_context.background = image

        event_response = ResponseFactory.create_background(image, duration = duration)
        await message_broker.publish("1",
            event_response.model_dump()
        )

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'background'
