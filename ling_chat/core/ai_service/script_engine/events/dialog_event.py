from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger

class DialogueEvent(BaseEvent):
    """处理对话事件"""
    
    async def execute(self):
        character = self.event_data.get('character', '')
        text = self.event_data.get('text', '')
        
        logger.info(f"显示对话: {character} - {text}")
        
        # 在实际实现中，这里会更新游戏状态和UI
        self.game_context.dialogue.append({
            'character': character,
            'text': text,
        })
        
        # TODO 等待玩家点击继续 
        # await self.game_context.wait_for_click()
    
    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'dialogue'