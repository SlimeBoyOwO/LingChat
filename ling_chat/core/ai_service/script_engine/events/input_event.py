from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.logger import logger
from ling_chat.core.schemas.response_models import ResponseFactory
from typing import Any, Dict

class InputEvent(BaseEvent):
    """处理输入对话信息事件"""
    
    async def execute(self):
        hint: str = self.event_data.get('hint', '')
        duration: float = self.event_data.get('duration', 0.0)
        logger.info(f"InputEvent: {hint}")

        # 推送前端需要输入的事件
        event_response = ResponseFactory.create_input(hint, duration=duration)
        await message_broker.publish("1", event_response.model_dump())

        # 等待来自前端的输入
        user_input = await self._wait_for_user_input()
        
        # 将用户输入存储到游戏上下文
        self.game_context.dialogue.append({
            'player': "player",
            'text': user_input,
        })
        
        logger.info(f"用户输入已接收并存储: {user_input}")

    async def _wait_for_user_input(self) -> str | None:
        """等待来自前端的用户输入"""
        # TODO: 获取客户端id
        client_id = "1"
        try:
            # 订阅特定的输入频道
            subscription = message_broker.subscribe("ai_script_input_" + client_id)
            
            # 使用异步for循环来消费消息
            async for message in subscription:
                user_input = self._extract_user_input(message)
                if user_input:
                    return user_input
                    
        except Exception as e:
            logger.error(f"等待用户输入时发生错误: {e}")
            return ""

    def _extract_user_input(self, message: Dict[str, Any]) -> str:
        """从消息中提取用户输入文本"""
        try:
            # 根据实际的消息结构来提取用户输入
            # 这里假设消息中有 'text' 或 'input' 字段包含用户输入
            if isinstance(message, dict):
                return message.get('content','')
            else:
                return str(message)
        except Exception as e:
            logger.error(f"提取用户输入时发生错误: {e}")
            return ""

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == 'input'