from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.game_database.models import LineAttribute, LineBase


class InputEvent(BaseEvent):
    """处理输入对话信息事件"""

    async def execute(self):
        hint: str = self.event_data.get('hint', '')
        duration: float = self.event_data.get('duration', 0.0)
        logger.info(f"InputEvent: {hint}")

        # 推送前端需要输入的事件
        event_response = ResponseFactory.create_input(hint, duration=duration)
        await message_broker.publish(self.client_id, event_response.model_dump())

        # 等待来自前端的输入
        user_input = await ScriptFunction.wait_for_user_input(self.client_id)

        # 将用户输入存储到游戏上下文
        if user_input is not None:
            self.game_status.add_line(
                LineBase(content=user_input,attribute=LineAttribute.USER,display_name=self.game_status.player.user_name)
            )
        else:
            logger.warning("剧本输入事件中用户未输入任何内容")

        logger.info(f"用户输入已接收并存储: {user_input}")

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'input'
