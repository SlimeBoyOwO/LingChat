from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager
from ling_chat.core.messaging.broker import message_broker
from ling_chat.game_database.models import LineAttribute, LineBase


class AIDialogueEvent(BaseEvent):
    """处理AI对话事件"""

    async def execute(self):
        character = self.event_data.get('character', '')
        prompt = self.event_data.get('prompt', '')

        role = ScriptFunction.get_role(self.game_status, self.script_status, character)
        self.game_status.current_character = role

        # TODO： 把  prompt 加入到 game_status 的 Line 中，作为提示词
        # 1. 获取最后一个 user 类型的 Line， 把 prompt 添加到里面的 content 中？
        # 2. 其实不对，我建议是，手动调用刷新记忆功能，把 prompt 添加到 memory 中
        if prompt and prompt != '':
            system_input = LineBase(content=ScriptFunction.user_message_builder("", prompt),attribute=LineAttribute.USER,display_name=self.game_status.player.user_name)
            self.game_status.add_line(system_input)

        ai_service = service_manager.ai_service
        if not ai_service:
            logger.error("AI service not found")
            return

        async for response in ai_service.message_generator.process_message_stream():
            await message_broker.publish(self.client_id, response.model_dump())


    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'ai_dialogue'
