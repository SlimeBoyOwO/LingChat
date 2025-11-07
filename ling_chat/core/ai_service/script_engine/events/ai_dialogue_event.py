from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger

from ling_chat.core.service_manager import service_manager

class AIDialogueEvent(BaseEvent):
    """处理AI对话事件"""
    
    async def execute(self):
        character = self.event_data.get('character', '')
        prompt = self.event_data.get('prompt', '')

        # 首先，获取角色的 memory 信息
        character_obj = self.game_context.characters.get(character)
        if not character_obj:
            logger.error(f"Character memory not found for character: {character}")
            return
        
        memory = character_obj.memory.copy()

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
        
    
    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'ai_dialogue'