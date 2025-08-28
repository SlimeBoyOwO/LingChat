from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.logger import logger

class DialogueEvent(BaseEvent):
    """处理对话事件"""
    
    async def execute(self):
        character = self.event_data.get('character', '')
        text = self.event_data.get('text', '')
        
        logger.info(f"显示对话: {character} - {text}")

        # TODO: 获取角色语音模型，用translator翻译成对应语言，打包发送完整AI消息
        
        self.game_context.dialogue.append({
            'character': character,
            'text': text,
        })
    
    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'dialogue'