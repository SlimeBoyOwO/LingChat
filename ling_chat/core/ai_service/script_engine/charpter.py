from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.script_engine.type import GameContext
from ling_chat.core.logger import logger

from .ends_handler import EndsHandler
from .events_handler import EventsHandler


class Charpter:
    def __init__(self, charpter_id: str, config: AIServiceConfig, game_context: GameContext, events_data: list[dict], ends_data: dict):
        self.charpter_id = charpter_id

        # 章节内部持有自己的处理器，状态被封装在内部
        self.game_context = game_context
        self._events_handler = EventsHandler(config, events_data, game_context)
        self._ends_handler = EndsHandler(ends_data, game_context)

        logger.info(f"章节 '{self.charpter_id}' 已初始化。")

    async def run(self) -> str:
        """
        运行本章节的所有事件，并返回下一章节的名称。
        这是章节的核心行为。
        """
        logger.info(f"开始执行章节: {self.charpter_id}")

        # 1. 驱动事件处理器，直到所有事件处理完毕
        while not self._events_handler.is_finished():
            await self._events_handler.process_next_event()

        logger.info(f"章节 '{self.charpter_id}' 的所有事件已处理完毕。")

        # 2. 调用结局处理器，获取结果
        next_charpter_name = self._ends_handler.process_end()

        logger.info(f"章节 '{self.charpter_id}' 的下一章节是: {next_charpter_name}")

        # 3. 将下一章节的名称返回给调用者 (ScriptManager)
        return next_charpter_name
