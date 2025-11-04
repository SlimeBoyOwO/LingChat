import asyncio
from typing import List, Dict, Optional
import time
import traceback

from ling_chat.core.ai_service.ai_logger import logger
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.logger import logger

from ling_chat.core.schemas.responses import ReplyResponse
from ling_chat.core.schemas.response_models import ResponseFactory

class SentenceConsumer:
    """
    Consumes sentences from a queue, processes them, and stores the results.
    """
    def __init__(self,
                 consumer_id: int,
                 sentence_queue: asyncio.Queue,
                 results_store: Dict[int, ReplyResponse],
                 publish_events: Dict[int, asyncio.Event],
                 message_processor: MessageProcessor,
                 translator: Translator,
                 voice_maker: VoiceMaker,
                 user_message: str,
                 character: str = "default"):
        self.consumer_id = consumer_id
        self.sentence_queue = sentence_queue
        self.results_store = results_store
        self.publish_events = publish_events
        self.message_processor = message_processor
        self.translator = translator
        self.voice_maker = voice_maker
        self.user_message = user_message
        self.character = character

    async def run(self):
        """Starts the consumer loop."""
        while True:
            try:
                task = await self.sentence_queue.get()
                if task is None:  # End signal
                    break

                sentence, index, is_final = task
                response = await self._process_sentence_and_prepare_response(sentence, self.user_message, is_final)
                
                # If processing produced no response, skip storing but still mark task done and notify if needed.
                if response is None:
                    logger.warning(f"Consumer {self.consumer_id} returned no response for index {index}.")
                else:
                    # Store the result and notify the publisher
                    self.results_store[index] = response
                    if index in self.publish_events:
                        self.publish_events[index].set()
                
                # TODO: 还需要返回这个response才对
                
                self.sentence_queue.task_done()
            except asyncio.CancelledError:
                logger.info(f"Consumer {self.consumer_id} was cancelled.")
                break
            except Exception as e:
                logger.error(f"Error in consumer {self.consumer_id}: {e}", exc_info=True)
                # 使用 traceback 模块获取详细的错误信息
                traceback.print_exc()
                self.sentence_queue.task_done()


    async def _process_sentence_and_prepare_response(self, sentence: str, user_message: str, is_final: bool) -> Optional[ReplyResponse]:
        """(Helper) Processes a single sentence and prepares the response dictionary."""
        # This logic is identical to your original helper method
        if not sentence:
            return None
        
        logger.info(f"Consumer {self.consumer_id} processing sentence: {sentence[:30]}...")
        sentence_segments:List[Dict] = self.message_processor.analyze_emotions(sentence)
        if not sentence_segments:
            logger.warning("AI response format error: No emotion or text found.")
            return None
        
        start_time = time.perf_counter()
        if sentence_segments[0].get("japanese_text") == "":
            await self.translator.translate_ai_response(sentence_segments)
        else:
            await self.voice_maker.generate_voice_files(sentence_segments)
        end_time = time.perf_counter()

        sentence_segments[0]['character'] = self.character
        
        # Assuming create_response is a method in the orchestrator or a utility class
        response = ResponseFactory.create_reply(sentence_segments[0], user_message, is_final)
        logger.debug(f"Sentence processed in {end_time - start_time:.2f} seconds.")
        return response