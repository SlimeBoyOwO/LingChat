from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.logger import logger

class BackgroundEvent(BaseEvent):
    """处理背景切换事件"""
    
    async def execute(self):
        image = self.event_data.get('image', '')
        
        logger.info(f"切换背景: {image}")
        
        # 更新游戏状态
        self.game_context.background = image
        
        # TODO: 向客户端发送背景切换事件
    
    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'background'