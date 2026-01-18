import asyncio
import os
import time
from typing import AsyncGenerator, Dict, List, Optional

from ling_chat.core.ai_service.ai_logger import AILogger, logger
from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.ai_service.message_system.response_publisher import (
    ResponsePublisher,
)
from ling_chat.core.ai_service.message_system.sentence_comsumer import SentenceConsumer
from ling_chat.core.ai_service.message_system.stream_producer import StreamProducer
from ling_chat.core.ai_service.rag_manager import RAGManager
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.logger import logger
from ling_chat.core.schemas.response_models import ResponseFactory
from ling_chat.core.schemas.responses import ReplyResponse
from ling_chat.utils.function import Function


class MessageGenerator:
    def __init__(self,
                config: AIServiceConfig,
                voice_maker: VoiceMaker,
                message_processor: MessageProcessor,
                translator: Translator,
                llm_model: LLMManager,
                rag_manager: Optional[RAGManager],
                ai_logger: AILogger):
        self.config = config
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_manager = rag_manager if rag_manager else RAGManager() if self.use_rag else None
        self.voice_maker = voice_maker if voice_maker else VoiceMaker()
        self.message_processor = message_processor if message_processor else MessageProcessor(self.voice_maker)
        self.translator = translator if translator else Translator(self.voice_maker)
        self.llm_model = llm_model if llm_model else LLMManager()
        self.ai_logger = ai_logger if ai_logger else AILogger()
        self.function = Function()
        self.concurrency = int(os.environ.get("COMSUMERS", 3))

    def memory_init(self, memory: List[Dict]) -> None:
        self.memory = memory

    async def process_sentence(self, sentence: str, emotion_segments: List[Dict]):
        """处理单个句子的情绪分析、翻译和语音合成"""
        if not sentence:
            return

         # 使用analyze_emotions处理句子 返回情绪-中文-日文等信息
        sentence_segments:List[Dict] = self.message_processor.analyze_emotions(sentence)
        if not sentence_segments:
            logger.warning("句子中没有出现中日或情感，AI回复格式错误")
            return
        else:
            # 翻译句子 TODO 假如翻译句子用的是比较贵的AI，这里不应该每个句子都单独飞过去翻译
            start_time = time.perf_counter()
            if sentence_segments[0].get("japanese_text") == "":
                await self.translator.translate_ai_response(sentence_segments)
            else:
                await self.voice_maker.generate_voice_files(sentence_segments)
            end_time = time.perf_counter()
            # 更新情绪片段列表
            emotion_segments.extend(sentence_segments)

            logger.debug(f"句子处理时间: {end_time - start_time} 秒")

    # 主方法现在充当协调器角色
    async def process_message_stream(self, user_message: str, character: str = "default", memory: Optional[List[Dict]] = None) -> AsyncGenerator[ReplyResponse, None]:
        """
        协调流处理管道并生成响应，避免死锁。
        """
        rag_messages = []
        processed_user_message = ""
        temp_message = None
        # 1. 设置和预处理
        current_context = self.memory.copy() if not memory else memory.copy()

        if not memory:
            processed_user_message_dict = self.message_processor.append_user_message(user_message)
            processed_user_message = processed_user_message_dict.get("main","")
            temp_message = processed_user_message_dict.get("temp",None)

            self.memory.append({"role": "user", "content": processed_user_message})
            current_context = self.memory.copy()
            if self.use_rag and self.rag_manager:
                self.rag_manager.rag_append_sys_message(current_context, rag_messages, processed_user_message)

            if logger.should_print_context():
                self.ai_logger.print_debug_message(current_context, rag_messages, self.memory)

        # 2. 管道组件的共享状态
        sentence_queue = asyncio.Queue(maxsize=self.concurrency * 2)
        results_store: Dict[int, ReplyResponse] = {}
        publish_events: Dict[int, asyncio.Event] = {}
        output_queue = asyncio.Queue()

        # 用于优雅管理所有后台任务的列表
        background_tasks = []
        accumulated_response = ""

        try:
            # 3. 实例化并启动所有管道组件作为后台任务

            # 发布者任务
            publisher = ResponsePublisher(results_store, publish_events, output_queue)
            publisher_task = asyncio.create_task(publisher.run(), name="Publisher")
            background_tasks.append(publisher_task)

            # 消费者任务
            for i in range(self.concurrency):
                consumer = SentenceConsumer(
                    consumer_id=i, sentence_queue=sentence_queue,
                    results_store=results_store, publish_events=publish_events,
                    message_processor=self.message_processor, translator=self.translator,
                    voice_maker=self.voice_maker, user_message=user_message,
                    character=character
                )
                consumer_task = asyncio.create_task(consumer.run(), name=f"Consumer-{i}")
                background_tasks.append(consumer_task)

            # 生产者任务：立即将生产者作为后台任务启动
            ai_response_stream = self.llm_model.process_message_stream(current_context)
            producer = StreamProducer(ai_response_stream, sentence_queue, publish_events)
            producer_task = asyncio.create_task(producer.run(), name="Producer")
            background_tasks.append(producer_task)

            # 4. 现在，主协程的工作是从管道生成结果
            # 需要同时监听 producer_task 以便捕获 LLM 异常
            while True:
                # 创建一个获取队列的任务
                queue_get_task = asyncio.create_task(output_queue.get())

                # 同时等待队列和producer任务，看哪个先完成
                done, pending = await asyncio.wait(
                    [queue_get_task, producer_task],
                    return_when=asyncio.FIRST_COMPLETED
                )

                # 检查 producer_task 是否出错
                if producer_task in done:
                    # 取消队列获取任务
                    queue_get_task.cancel()
                    try:
                        await queue_get_task
                    except asyncio.CancelledError:
                        pass

                    # 检查 producer 是否有异常
                    producer_exception = producer_task.exception()
                    if producer_exception is not None:
                        raise producer_exception

                    # 如果 producer 正常完成但队列还没有数据，继续等待队列
                    if queue_get_task not in done:
                        response = await output_queue.get()
                    else:
                        response = queue_get_task.result()
                else:
                    response = queue_get_task.result()

                yield response
                # 当收到最终消息时循环自然结束
                if response.isFinal:
                    break

            # 5. 优雅关闭和后续处理

            # 现在可以安全地等待producer_task以获取完整响应
            # 当上面的while循环完成时，生产者任务也必须完成
            accumulated_response = await producer_task

            # 等待消费者完成队列中的任何剩余项目
            await sentence_queue.join()

            # 向消费者发送停止信号
            for _ in range(self.concurrency):
                await sentence_queue.put(None)

            # 发布者任务在发送最终消息后应该已经完成
            # 我们在finally块中等待所有任务以进行清理

            # 6. 后续处理
            if accumulated_response:
                if not memory:

                    # 让 processed_user_message 删除 temp_message 字段
                    if temp_message is not None:
                        self.memory.pop()
                        processed_user_message = processed_user_message.replace(temp_message, "")
                        self.memory.append({"role": "user", "content": processed_user_message})

                    self.memory.append({"role": "assistant", "content": accumulated_response})
                    if self.use_rag and self.rag_manager:
                        self.rag_manager.save_messages_to_rag(self.memory)
                self.ai_logger.log_conversation("钦灵", accumulated_response)
            else:
                self.ai_logger.log_conversation("钦灵", "未生成响应。")


        except Exception as e:
            logger.error(f"消息流管道中发生错误: {e}", exc_info=True)

            # 准备错误代码（前端负责翻译）
            from ling_chat.core.messaging.broker import message_broker
            error_message = str(e)
            error_code = "default_error"  # 默认错误代码

            # 检查错误类型，确定错误代码
            if "401" in error_message or "Api key is invalid" in error_message or "AuthenticationError" in str(type(e)):
                error_code = "401"
            elif "404" in error_message:
                error_code = "404"
            elif "网络" in error_message or "connection" in error_message.lower():
                error_code = "network_error"

            # 1. 发送错误代码到前端（由前端翻译显示弹窗）
            error_data = {
                "type": "error",
                "error_code": error_code,
                "detail": str(e)  # 原始错误信息，用于调试
            }

            for client_id in self.config.clients:
                await message_broker.publish(client_id, error_data)

            # 2. 发送状态重置消息，让前端回到输入状态
            reset_data = {
                "type": "status_reset",
                "status": "input"
            }
            for client_id in self.config.clients:
                await message_broker.publish(client_id, reset_data)

            # 3. 同时生成错误响应对象（供后端调用方使用）
            error_response = ResponseFactory.create_error_reply(str(e))
            yield error_response
        finally:
            # 7. 最终清理：取消任何可能仍在运行的任务
            for task in background_tasks:
                if not task.done():
                    task.cancel()
            await asyncio.gather(*background_tasks, return_exceptions=True)
            logger.info("消息流处理完成，所有任务已清理完毕。")
