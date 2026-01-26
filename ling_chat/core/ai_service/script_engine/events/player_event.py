from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.game_database.models import LineAttribute, LineBase


class PlayerEvent(BaseEvent):
    """处理玩家对话事件"""

    async def execute(self):
        text:str = self.event_data.get('text', '')
        lines: list[str] = [line for line in text.splitlines() if line.strip()]

        for text in lines:
            logger.info(f"显示对话: 玩家 - {text}")

            # 在实际实现中，这里会更新游戏状态和UI
            self.game_status.add_line(
                LineBase(content=text,attribute=LineAttribute.USER,display_name=self.game_status.player.user_name)
            )

            event_response = ResponseFactory.create_player_dialogue(text)
            await message_broker.publish(self.client_id,
                event_response.model_dump()
            )

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'player'
