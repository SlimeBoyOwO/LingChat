from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.service_manager import service_manager
from ling_chat.game_database.models import LineAttribute, LineBase


class AIDialogueEvent(BaseEvent):
    """处理AI对话事件"""

    async def _execute(self):
        character = self.event_data.get("character", "")
        prompt = self.event_data.get("prompt", "")
        prompt = ScriptFunction.replace_placeholder(
            prompt, self.game_status, self.script_status
        )

        role = ScriptFunction.get_role(self.game_status, self.script_status, character)
        self.game_status.current_character = role

        if prompt and prompt != "":
            system_input = LineBase(
                content=ScriptFunction.user_message_builder("", prompt),
                attribute=LineAttribute.USER,
                display_name=self.game_status.player.user_name,
            )
            self.game_status.add_line(system_input)

        ai_service = service_manager.ai_service
        if not ai_service:
            logger.error("AI service not found")
            return

        # TODO：获取上一个事件是否为玩家输入再执行这个，会比较优雅
        # await message_broker.publish(self.client_id, (ResponseFactory.create_thinking(True).model_dump()))
        async for response in ai_service.message_generator.process_message_stream():
            await message_broker.publish(self.client_id, response.model_dump())

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == "ai_dialogue"
