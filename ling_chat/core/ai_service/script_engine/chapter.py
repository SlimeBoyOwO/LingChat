from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.type import ScriptStatus
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.schemas.response_models import ResponseFactory

from .events_handler import EventsHandler


class Chapter:
    def __init__(self, charpter_id: str, config: AIServiceConfig, game_status: GameStatus, chapter_config: dict, script_status:ScriptStatus):
        self.chapter_id = charpter_id
        self.chapter_name = chapter_config.get("name", "")

        # 章节内部持有自己的处理器，状态被封装在内部
        self.game_status = game_status
        events_data:list[dict] = chapter_config.get('events',[])
        self._events_handler = EventsHandler(config, events_data, game_status)

        self.client_id = script_status.running_client_id

        logger.info(f"章节 '{self.chapter_id}' 已初始化。")

    async def run(self) -> str:
        logger.info(f"开始执行章节: {self.chapter_id}")

        # 发送章节信息发送事件
        event_response = ResponseFactory.create_chapter_change(self.chapter_name)
        if self.client_id:
            await message_broker.publish(self.client_id,
                event_response.model_dump()
            )
        else:
            logger.warning("没有找到客户端ID，无法发送章节信息。")

        # 驱动事件处理器，直到处理完毕
        while not self._events_handler.is_finished():
            await self._events_handler.process_next_event()

        # 从处理器获取章节结果
        next_chapter = self._events_handler.get_chapter_result()
        
        logger.info(f"章节 '{self.chapter_id}' 结束，下一章: {next_chapter}")
        return next_chapter
