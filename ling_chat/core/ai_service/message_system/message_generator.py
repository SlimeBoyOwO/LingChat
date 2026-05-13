import asyncio
import os
import time
from typing import AsyncGenerator, Dict, List, Optional

from ling_chat.core.ai_service.ai_logger import AILogger, logger
from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.core.ai_service.message_system.message_processor import MessageProcessor
from ling_chat.core.ai_service.message_system.response_publisher import (
    ResponsePublisher,
)
from ling_chat.core.ai_service.message_system.sentence_comsumer import SentenceConsumer
from ling_chat.core.ai_service.message_system.stream_producer import StreamProducer
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.logger import logger
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.core.schemas.responses import ReplyResponse
from ling_chat.game_database.models import LineAttribute, LineBase
from ling_chat.utils.function import Function


class MessageGenerator:
    def __init__(
        self,
        config: AIServiceConfig,
        message_processor: MessageProcessor,
        translator: Translator,
        llm_model: LLMManager,
        ai_logger: AILogger,
        game_status: GameStatus,
    ):
        self.config = config
        self.message_processor = message_processor
        self.translator = translator
        self.llm_model = llm_model
        self.ai_logger = ai_logger
        self.function = Function()
        self.game_status = game_status
        self.concurrency = int(os.environ.get("COMSUMERS", 3))

    async def process_sentence(self, sentence: str, emotion_segments: List[Dict]):
        if not sentence:
            return

        sentence_segments: List[Dict] = (
            self.message_processor.parse_and_classify_emotional_segments(sentence)
        )
        if not sentence_segments:
            logger.warning("句子中没有出现中日或情感，AI回复格式错误")
            return
        else:
            start_time = time.perf_counter()
            if sentence_segments[0].get("japanese_text") == "":
                await self.translator.translate_ai_response(sentence_segments)
            else:
                if self.game_status.current_character:
                    await self.game_status.current_character.voice_maker.generate_voice_files(
                        sentence_segments
                    )
            end_time = time.perf_counter()
            emotion_segments.extend(sentence_segments)

            logger.debug(f"句子处理时间: {end_time - start_time} 秒")

    async def process_message_stream(
        self,
        user_message: Optional[str] = None,
        memory: Optional[List[Dict]] = None,
    ) -> AsyncGenerator[ReplyResponse, None]:
        rag_messages = []
        processed_user_message = ""
        temp_message = None
        current_context = []

        line = None
        if user_message is not None:
            processed_user_message_dict = (
                await self.message_processor.append_user_message(user_message)
            )
            processed_user_message = processed_user_message_dict.get("main", "")
            temp_message = processed_user_message_dict.get("temp", None)
            line = LineBase(
                content=processed_user_message,
                attribute=LineAttribute.USER,
                display_name=self.game_status.player.user_name,
            )
            self.game_status.add_line(line)

        role = self.game_status.current_character
        if role:
            current_context = role.memory.copy()
        elif memory:
            current_context = memory.copy()
        else:
            logger.error("生成消息的时候没有当前角色或者记忆，取消生成消息")
            return

        if logger.should_print_context():
            self.ai_logger.print_debug_message(
                current_context, rag_messages, current_context
            )

        sentence_queue = asyncio.Queue(maxsize=self.concurrency * 2)
        results_store: Dict[int, ReplyResponse] = {}
        publish_events: Dict[int, asyncio.Event] = {}
        output_queue = asyncio.Queue()

        background_tasks = []
        accumulated_response = ""

        try:
            publisher = ResponsePublisher(results_store, publish_events, output_queue)
            publisher_task = asyncio.create_task(publisher.run(), name="Publisher")
            background_tasks.append(publisher_task)

            for i in range(self.concurrency):
                consumer = SentenceConsumer(
                    consumer_id=i,
                    sentence_queue=sentence_queue,
                    results_store=results_store,
                    publish_events=publish_events,
                    message_processor=self.message_processor,
                    translator=self.translator,
                    user_message=user_message if user_message else "",
                    game_status=self.game_status,
                )
                consumer_task = asyncio.create_task(
                    consumer.run(), name=f"Consumer-{i}"
                )
                background_tasks.append(consumer_task)

            ai_response_stream = self.llm_model.process_message_stream(current_context)
            producer = StreamProducer(
                ai_response_stream, sentence_queue, publish_events
            )
            producer_task = asyncio.create_task(producer.run(), name="Producer")
            background_tasks.append(producer_task)

            while True:
                queue_get_task = asyncio.create_task(output_queue.get())

                done, pending = await asyncio.wait(
                    [queue_get_task, producer_task], return_when=asyncio.FIRST_COMPLETED
                )

                if producer_task in done:
                    queue_get_task.cancel()
                    try:
                        await queue_get_task
                    except asyncio.CancelledError:
                        pass

                    producer_exception = producer_task.exception()
                    if producer_exception is not None:
                        raise producer_exception

                    if queue_get_task not in done:
                        response = await output_queue.get()
                    else:
                        response = queue_get_task.result()
                else:
                    response = queue_get_task.result()

                yield response
                if response.isFinal:
                    break

            accumulated_response = await producer_task

            try:
                cleanup_timeout = max(1, int(os.environ.get("PIPELINE_CLEANUP_TIMEOUT", "10")))
            except (ValueError, TypeError):
                cleanup_timeout = 10
            try:
                await asyncio.wait_for(sentence_queue.join(), timeout=cleanup_timeout)
            except asyncio.TimeoutError:
                logger.warning(
                    f"消费者处理超时（>{cleanup_timeout}s），跳过剩余队列，强制进入清理"
                )
                from ling_chat.core.messaging.broker import message_broker

                timeout_msg = {
                    "type": "error",
                    "error_code": "voice_timeout",
                    "detail": f"语音合成超时（>{cleanup_timeout}s），已跳过剩余语音生成",
                }
                for client_id in self.config.clients:
                    await message_broker.publish(client_id, timeout_msg)

            for _ in range(self.concurrency):
                await sentence_queue.put(None)

            ai_name = ""
            if self.game_status.current_character:
                ai_name = self.game_status.current_character.display_name
            if not ai_name:
                ai_name = "Nameless"
            if accumulated_response:
                if temp_message is not None and line is not None:
                    line.content = processed_user_message.replace(temp_message, "")
                    self.game_status.refresh_memories()

                self.ai_logger.log_conversation(ai_name, accumulated_response)
            else:
                self.ai_logger.log_conversation(ai_name, "未生成响应。")

        except Exception as e:
            logger.error(f"消息流管道中发生错误: {e}", exc_info=True)

            from ling_chat.core.messaging.broker import message_broker

            error_message = str(e)
            error_code = "default_error"

            if (
                "401" in error_message
                or "Api key is invalid" in error_message
                or "AuthenticationError" in str(type(e))
            ):
                error_code = "401"
            elif "404" in error_message:
                error_code = "404"
            elif "网络" in error_message or "connection" in error_message.lower():
                error_code = "network_error"

            error_data = {
                "type": "error",
                "error_code": error_code,
                "detail": str(e),
            }

            for client_id in self.config.clients:
                await message_broker.publish(client_id, error_data)

            reset_data = {"type": "status_reset", "status": "input"}
            for client_id in self.config.clients:
                await message_broker.publish(client_id, reset_data)

            error_response = ResponseFactory.create_error_reply(str(e))
            yield error_response
        finally:
            for task in background_tasks:
                if not task.done():
                    task.cancel()
            await asyncio.gather(*background_tasks, return_exceptions=True)
            logger.info("消息流处理完成，所有任务已清理完毕。")
