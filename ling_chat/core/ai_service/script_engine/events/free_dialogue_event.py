from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.core.service_manager import service_manager


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

        while True:
            is_last_round: bool = False

            rounds += 1
            if max_rounds > 0 and rounds >= max_rounds:
                is_last_round = True

            # 推送前端需要输入的事件
            event_response = ResponseFactory.create_input(hint,duration=duration)
            await message_broker.publish("1", event_response.model_dump())

            # 等待来自前端的输入
            user_input = await ScriptFunction.wait_for_user_input()

            # 将用户输入存储到游戏上下文
            self.game_context.dialogue.append({
                'character': "player",
                'text': user_input,
            })

            if end_line and user_input == end_line:
                is_last_round = True

            # 让 AI 回复，然后直到达到最大轮数或结束语
            # 首先，获取角色的 memory 信息
            character_obj = self.game_context.characters.get(character)
            if not character_obj:
                logger.error(f"Character memory not found for character: {character}")
                return

            memory = character_obj.memory.copy()
            prompt = end_prompt if is_last_round else dialog_prompt

            ScriptFunction.memory_builder(self.game_context, memory, character, prompt)

            # 然后，使用 memory 信息生成对话
            ai_service = service_manager.ai_service
            if not ai_service:
                logger.error("AI service not found")
                return

            logger.info(f"AI Dialogue Event for character: {character} with memory: {memory}")

            responses = []
            async for response in ai_service.message_generator.process_message_stream("",character=character,memory=memory):
                responses.append(response)

            for response in responses:
                text = "【" + response.originalTag + "】" + response.message + ("(" + response.motionText + ")" if response.motionText else "")
                self.game_context.dialogue.append({
                    'character': character,
                    'text': text,
                })

            responses.clear()

            if is_last_round:
                break

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'free_dialogue'
