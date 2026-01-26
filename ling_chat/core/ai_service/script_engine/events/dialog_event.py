from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.core.service_manager import service_manager
from ling_chat.game_database.models import LineAttribute, LineBase
from ling_chat.utils.function import Function


class DialogueEvent(BaseEvent):
    """处理对话事件"""

    async def execute(self):
        character = self.event_data.get('character', '')
        text = self.event_data.get('text', '')

        role = ScriptFunction.get_role(self.game_status, self.script_status, character)
        self.game_status.current_character = role

        lines: list[str] = [line for line in text.splitlines() if line.strip()]
        for text in lines:
            logger.info(f"显示对话: {character} - {text}")

            seg:list[dict] = []
            ai_service = service_manager.ai_service
            if not ai_service: return

            await ai_service.message_generator.process_sentence(text, seg)
            seg[0]['character'] = character
            seg[0]['role_id'] = role.role_id

            event_response = ResponseFactory.create_reply(seg[0], "", False)

            self.game_status.add_line(
                Function.convert_reply_to_line(event_response)
            )

            await message_broker.publish(self.client_id,
                event_response.model_dump()
            )

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'dialogue'
