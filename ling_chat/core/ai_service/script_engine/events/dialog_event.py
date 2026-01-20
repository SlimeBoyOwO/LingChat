from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.core.service_manager import service_manager


class DialogueEvent(BaseEvent):
    """处理对话事件"""

    async def execute(self):
        character = self.event_data.get('character', '')
        text = self.event_data.get('text', '')

        lines: list[str] = [line for line in text.splitlines() if line.strip()]
        for text in lines:
            logger.info(f"显示对话: {character} - {text}")

            self.game_context.dialogue.append({
                'character': character,
                'text': text,
            })

            seg:list[dict] = []
            ai_service = service_manager.ai_service
            if not ai_service: return

            await ai_service.message_generator.process_sentence(text, seg)
            seg[0]['character'] = character

            event_response = ResponseFactory.create_reply(seg[0], "", False)
            await message_broker.publish("1",
                event_response.model_dump()
            )

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'dialogue'
