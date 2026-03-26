from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.game_database.models import LineAttribute, LineBase


class ChoiceEvent(BaseEvent):
    """弹出选项事件"""

    async def _execute(self):
        options = self.event_data.get('options', [])
        allow_free = self.event_data.get('allow_free', False)
        choices = []
        for option in options:
            option_text = option.get('text', '')
            choices.append(option_text)

        # 推送前端需要输入的事件
        event_response = ResponseFactory.create_choice(choices, allow_free)
        await message_broker.publish(self.client_id, event_response.model_dump())

        # 等待来自前端的输入
        choice_input = await ScriptFunction.wait_for_user_input(self.client_id)

        # TODO: 等待更优雅的发言思考者判断重构
        await message_broker.publish(self.client_id, (ResponseFactory.create_thinking(True).model_dump()))

        match = await ScriptFunction.process_options(self.game_status,self.script_status,options,choice_input)
        
        # 如果没有匹配的选项，则直接把用户输入的本台词加入到对话中
        if not match:
            user_input = choice_input
            if user_input:
                self.game_status.add_line(
                    LineBase(content=user_input,attribute=LineAttribute.USER,display_name=self.game_status.player.user_name)
                )
        

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'choices'
