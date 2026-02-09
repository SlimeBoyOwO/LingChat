from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.core.service_manager import service_manager
from ling_chat.game_database.models import LineAttribute, LineBase


class FreeDialogueEvent(BaseEvent):
    """自由对话事件"""

    async def execute(self):
        rounds: int = 0

        character: str = self.event_data.get('character', 'default')
        hint: str = self.event_data.get('hint', '')
        max_rounds: int = self.event_data.get('max_rounds', -1)
        end_line: str = self.event_data.get('end_line', '')
        dialog_prompt: str = self.event_data.get('prompt', '')
        end_prompt: str = self.event_data.get('end_prompt', '')
        duration: float = self.event_data.get('duration', 0.0)   # 默认这类事件不给予等待时间

        role = ScriptFunction.get_role(self.game_status, self.script_status, character)
        self.game_status.current_character = role

        while True:
            is_last_round: bool = False

            # 让 AI 回复，然后直到达到最大轮数或结束语
            rounds += 1
            if max_rounds > 0 and rounds >= max_rounds:
                is_last_round = True

            # 推送前端需要输入的事件
            event_response = ResponseFactory.create_input(hint,duration=duration)
            await message_broker.publish(self.client_id, event_response.model_dump())

            # 等待来自前端的输入
            prompt = end_prompt if is_last_round else dialog_prompt
            user_input = await ScriptFunction.wait_for_user_input(self.client_id)
            extra_user_message = ("\n{剧情提示: " + prompt + "}") if prompt else ""

            # 将用户输入（加上剧情提示）存储到游戏上下文
            if user_input is not None:
                if extra_user_message != "": user_input += extra_user_message
                self.game_status.add_line(
                    LineBase(content=user_input,attribute=LineAttribute.USER,display_name=self.game_status.player.user_name)
                )
            else:
                logger.warning("剧本输入事件中用户未输入任何内容")

            if end_line and user_input == end_line:
                is_last_round = True

            # TODO: 这样可以是可以就是不太优雅，之后可以考虑优化一下
            ai_service = service_manager.ai_service
            if not ai_service:
                logger.error("AI 服务未初始化")
                return
            
            async for response in ai_service.message_generator.process_message_stream():
              await message_broker.publish(self.client_id, response.model_dump())

            if is_last_round:
                break

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'free_dialogue'
