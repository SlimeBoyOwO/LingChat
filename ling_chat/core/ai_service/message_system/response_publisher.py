import asyncio
from typing import List, Dict, Optional

from ling_chat.core.ai_service.ai_logger import logger
from ling_chat.core.logger import logger

from ling_chat.core.messaging.broker import message_broker

from ling_chat.core.schemas.responses import ReplyResponse


class ResponsePublisher:
    """
    Waits for processed results in sequence and publishes them.
    """
    def __init__(self, 
                 results_store: Dict[int, ReplyResponse], 
                 publish_events: Dict[int, asyncio.Event],
                 output_queue: asyncio.Queue):
        self.results_store = results_store
        self.publish_events = publish_events
        self.output_queue = output_queue

    async def run(self):
        """Starts the sequential publishing loop."""
        next_index_to_publish = 0
        while True:
            try:
                if next_index_to_publish not in self.publish_events:
                    await asyncio.sleep(0.01)
                    continue

                # Wait until the consumer signals that this result is ready
                await self.publish_events[next_index_to_publish].wait()
                
                response = self.results_store.pop(next_index_to_publish, None)
                if response:
                    logger.info(f"Publishing message index {next_index_to_publish}")
                    await self.output_queue.put(response)
                    await message_broker.publish("1", response.model_dump())
                
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