from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.logger import logger

from .events_handler import EventsHandler


class Chapter:
    def __init__(self, charpter_id: str, config: AIServiceConfig, game_status: GameStatus, events_data: list[dict], ends_data: dict):
        self.chapter_id = charpter_id

        # 章节内部持有自己的处理器，状态被封装在内部
        self.game_status = game_status
        self._events_handler = EventsHandler(config, events_data, game_status)

        logger.info(f"章节 '{self.chapter_id}' 已初始化。")

    async def run(self) -> str:
        logger.info(f"开始执行章节: {self.chapter_id}")

        # 驱动事件处理器，直到处理完毕
        while not self._events_handler.is_finished():
            await self._events_handler.process_next_event()

        # 从处理器获取章节结果
        next_chapter = self._events_handler.get_chapter_result()
        
        logger.info(f"章节 '{self.chapter_id}' 结束，下一章: {next_chapter}")
        return next_chapter
