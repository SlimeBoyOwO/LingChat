
from ling_chat.core.ai_service.script_engine.events.base_event import BaseEvent
from ling_chat.core.ai_service.script_engine.utils.script_function import ScriptFunction
from ling_chat.core.logger import logger


class SetVariableEvent(BaseEvent):
    """处理变量设置事件"""

    async def execute(self):
        options = self.event_data.get("options", [])
        if not options:
            logger.warning("SetVariableEvent 没有提供 options")
            return

        await ScriptFunction.process_options(
            self.game_status, self.script_status, options
        )

    @classmethod
    def can_handle(cls, event_type: str) -> bool:
        return event_type == "set_variable"
