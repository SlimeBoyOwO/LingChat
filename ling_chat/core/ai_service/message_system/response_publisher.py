import asyncio
from typing import Dict

from ling_chat.core.ai_service.ai_logger import logger
from ling_chat.core.logger import logger
from ling_chat.core.schemas.responses import ReplyResponse


class ResponsePublisher:
    """
    等待队列中的已处理结果并发布它们
    """
    def __init__(self,
                 results_store: Dict[int, ReplyResponse],
                 publish_events: Dict[int, asyncio.Event],
                 output_queue: asyncio.Queue):
        self.results_store = results_store
        self.publish_events = publish_events
        self.output_queue = output_queue

    async def run(self):
        """启动顺序发布循环"""
        next_index_to_publish = 0
        while True:
            try:
                if next_index_to_publish not in self.publish_events:
                    await asyncio.sleep(0.01)
                    continue

                # 等待消费者信号，表示此结果已准备好
                await self.publish_events[next_index_to_publish].wait()

                response = self.results_store.pop(next_index_to_publish, None)
                if response:
                    logger.info(f"Publishing message index {next_index_to_publish}")
                    await self.output_queue.put(response)
                    # await message_broker.publish(self.client_id, response.model_dump())

                del self.publish_events[next_index_to_publish]

                if response and response.isFinal:
                    logger.info("Final message published. Publisher is shutting down.")
                    break

                next_index_to_publish += 1
            except asyncio.CancelledError:
                logger.info("Publisher was cancelled.")
                break
            except Exception as e:
                logger.error(f"Error in publisher: {e}", exc_info=True)
                break
