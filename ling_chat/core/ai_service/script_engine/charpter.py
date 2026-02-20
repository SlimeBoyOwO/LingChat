from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.logger import logger

from .ends_handler import EndsHandler
from .events_handler import EventsHandler


class Chapter:
    def __init__(self, chapter_id: str, config: AIServiceConfig, game_status: GameStatus, events_data: list[dict], ends_data: dict):
        self.chapter_id = chapter_id

        # 章节内部持有自己的处理器，状态被封装在内部
        self.game_status = game_status
        self._events_handler = EventsHandler(config, events_data, game_status)
        self._ends_handler = EndsHandler(ends_data, game_status)

        logger.info(f"章节 '{self.chapter_id}' 已初始化。")

    async def run(self) -> str:
        """
        运行本章节的所有事件，并返回下一章节的名称。
        这是章节的核心行为。
        """
        logger.info(f"开始执行章节: {self.chapter_id}")

        # 1. 驱动事件处理器，直到所有事件处理完毕
        while not self._events_handler.is_finished():
            await self._events_handler.process_next_event()

        logger.info(f"章节 '{self.chapter_id}' 的所有事件已处理完毕。")

        # 2. 调用结局处理器，获取结果
        next_chapter_name = self._ends_handler.process_end()

        logger.info(f"章节 '{self.chapter_id}' 的下一章节是: {next_chapter_name}")

        # 3. 将下一章节的名称返回给调用者 (ScriptManager)
        return next_chapter_name
