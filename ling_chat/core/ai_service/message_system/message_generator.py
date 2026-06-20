import asyncio
import os
import time
from typing import AsyncGenerator, Dict, List, Optional

from ling_chat.core.agent_tools.runner import SimpleAgentRunner
from ling_chat.core.ai_service.ai_logger import AILogger
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
from ling_chat.core.messaging.broker import message_broker
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
        self.agent_runner = SimpleAgentRunner(llm_model, game_status)

    async def process_sentence(self, sentence: str, emotion_segments: List[Dict]):
        """处理单个句子的情绪分析、翻译和语音合成"""
        if not sentence:
            return

        # 使用analyze_emotions处理句子 返回情绪-中文-日文等信息
        sentence_segments: List[Dict] = (
            self.message_processor.parse_and_classify_emotional_segments(sentence)
        )
        if not sentence_segments:
            logger.warning("句子中没有出现中日或情感，AI回复格式错误")
            return
        else:
            # 翻译句子 TODO 假如翻译句子用的是比较贵的AI，这里不应该每个句子都单独飞过去翻译
            start_time = time.perf_counter()
            if sentence_segments[0].get("japanese_text") == "":
                await self.translator.translate_ai_response(sentence_segments)
            else:
                if self.game_status.current_character:
                    voice_maker = self.game_status.current_character.voice_maker
                    # 检查并重新合成缺失的语音文件
                    _, regenerated_count = await voice_maker.regenerate_missing_audio(
                        sentence_segments
                    )
                    if regenerated_count > 0:
                        logger.info(f"中文 TTS：成功重新合成 {regenerated_count} 条缺失语音")
            end_time = time.perf_counter()
            # 更新情绪片段列表
            emotion_segments.extend(sentence_segments)

            logger.debug(f"句子处理时间: {end_time - start_time} 秒")

    async def process_message_stream(
        self,
        user_message: Optional[str] = None,
        memory: Optional[List[Dict]] = None,
        client_id: Optional[str] = None,
        code_mode: bool = False,
        code_tts: bool = False,
    ) -> AsyncGenerator[ReplyResponse, None]:
        """
        协调流处理管道并生成响应，避免死锁。

        :param user_message: 用户消息文本
        :param memory: 可选的记忆列表
        :param client_id: 客户端 ID，用于 agent 工具调用
        :param code_mode: 是否为代码模式（启用沙箱和计划工具）
        :param code_tts: 代码模式下是否启用 TTS
        """
        rag_messages = []
        processed_user_message = ""
        temp_message = None
        # 1. 设置和预处理
        current_context = []

        line = None
        # 2. 处理用户消息，提取临时指令，构建台词
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

        # 3. 获取记忆
        role = self.game_status.current_character
        if role:
            current_context = role.memory.copy()
        elif memory:
            current_context = memory.copy()
        else:
            logger.error("生成消息的时候没有当前角色或者记忆，取消生成消息")
            return

        # 3.5 Agent 工具调用增强上下文（Code 模式下可用沙箱和计划工具）
        if user_message:
            current_context = await self.agent_runner.enrich_context_if_needed(
                current_context,
                user_message,
                client_id=client_id,
                code_mode=code_mode,
            )

        if logger.should_print_context():
            self.ai_logger.print_debug_message(
                current_context, rag_messages, current_context
            )

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

            # 生产者任务：立即将生产者作为后台任务启动
            ai_response_stream = self.llm_model.process_message_stream(current_context)
            producer = StreamProducer(
                ai_response_stream, sentence_queue, publish_events
            )
            producer_task = asyncio.create_task(producer.run(), name="Producer")
            background_tasks.append(producer_task)

            # 4. 现在，主协程的工作是从管道生成结果
            # 需要同时监听 producer_task 以便捕获 LLM 异常
            # 同时设置一个超时兜底，避免 publisher/consumer 出现意外死锁时
            # 主协程永远阻塞在 output_queue.get() 上 —— 那会让外层的
            # _generation_lock 永不释放，前端持续转圈无法恢复。
            pipeline_idle_timeout = float(
                os.environ.get("PIPELINE_IDLE_TIMEOUT", 90)
            )
            timed_out = False
            while True:
                # 创建一个获取队列的任务
                queue_get_task = asyncio.create_task(output_queue.get())

                # 同时等待队列和producer任务，看哪个先完成
                done, pending = await asyncio.wait(
                    [queue_get_task, producer_task],
                    return_when=asyncio.FIRST_COMPLETED,
                    timeout=pipeline_idle_timeout,
                )

                # 整体超时：done 为空说明这一轮里 producer 和 queue 都没产出任何东西
                # 这通常意味着 publisher/consumer 出现死锁。立刻取消 queue 等待，
                # yield 一个错误响应让外层退出循环并释放生成锁。
                if not done:
                    queue_get_task.cancel()
                    try:
                        await queue_get_task
                    except (asyncio.CancelledError, Exception):
                        pass
                    logger.error(
                        f"流式管道在 {pipeline_idle_timeout}s 内无任何输出，疑似死锁，"
                        "强制结束本轮生成以释放生成锁。"
                    )
                    timed_out = True
                    error_response = ResponseFactory.create_error_reply(
                        "AI 回复超时，请重试。"
                    )
                    yield error_response
                    break

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
                    # 同样要带超时，避免 publisher 死锁导致永远拿不到数据。
                    if queue_get_task not in done:
                        try:
                            response = await asyncio.wait_for(
                                output_queue.get(), timeout=pipeline_idle_timeout
                            )
                        except asyncio.TimeoutError:
                            logger.error(
                                f"producer 已完成但 publisher 在 {pipeline_idle_timeout}s 内"
                                "未产出任何剩余消息，强制结束本轮生成。"
                            )
                            timed_out = True
                            error_response = ResponseFactory.create_error_reply(
                                "AI 回复超时，请重试。"
                            )
                            yield error_response
                            break
                    else:
                        response = queue_get_task.result()
                else:
                    response = queue_get_task.result()

                yield response
                # 当收到最终消息时循环自然结束
                if response.isFinal:
                    break

            # 5. 优雅关闭和后续处理

            # 如果是被超时强制中断，跳过后续依赖 producer/consumer 的清理逻辑，
            # 让 finally 块统一取消所有后台任务即可。
            if timed_out:
                return

            # 现在可以安全地等待producer_task以获取完整响应
            # 当上面的while循环完成时，生产者任务也必须完成
            accumulated_response = await producer_task

            try:
                cleanup_timeout = max(1, int(os.environ.get("PIPELINE_CLEANUP_TIMEOUT", "10")))
            except (ValueError, TypeError):
                cleanup_timeout = 10
            try:
                await asyncio.wait_for(sentence_queue.join(), timeout=cleanup_timeout)
            except asyncio.TimeoutError:
                remaining = sentence_queue.qsize()
                logger.warning(
                    f"消费者处理超时（>{cleanup_timeout}s），跳过 {remaining} 个未处理句子，强制进入清理"
                )

                timeout_msg = {
                    "type": "error",
                    "error_code": "voice_timeout",
                    "detail": f"语音合成超时（>{cleanup_timeout}s），已跳过 {remaining} 个语音生成",
                }
                for client_id in self.config.clients:
                    await message_broker.publish(client_id, timeout_msg)

                # 清空队列并补偿 task_done 计数，确保后续 put(None) 不阻塞
                while not sentence_queue.empty():
                    try:
                        sentence_queue.get_nowait()
                        sentence_queue.task_done()
                    except asyncio.QueueEmpty:
                        break

            # 发布者任务在发送最终消息后应该已经完成
            # 我们在finally块中等待所有任务以进行清理

            # 向消费者发送停止信号
            for _ in range(self.concurrency):
                await sentence_queue.put(None)

            # 6. 后续处理
            ai_name = ""
            if self.game_status.current_character:
                ai_name = self.game_status.current_character.display_name
            if not ai_name:
                ai_name = "Nameless"
            if accumulated_response:
                # 让 processed_user_message 删除 temp_message 字段
                if temp_message is not None and line is not None:
                    line.content = processed_user_message.replace(temp_message, "")
                    # self.game_status.line_list[append_line_index] = line 这行应该没必要
                    self.game_status.refresh_memories()

                self.ai_logger.log_conversation(ai_name, accumulated_response)
            else:
                self.ai_logger.log_conversation(ai_name, "未生成响应。")

        except Exception as e:
            logger.error(f"消息流管道中发生错误: {e}", exc_info=True)

            # 准备错误代码（前端负责翻译）
            error_message = str(e)
            error_code = "default_error"  # 默认错误代码

            # 检查错误类型，确定错误代码
            if (
                "401" in error_message
                or "Api key is invalid" in error_message
                or "AuthenticationError" in str(type(e))
            ):
                error_code = "401"
            elif "404" in error_message:
                error_code = "404"
            elif "未初始化" in error_message or "未配置" in error_message:
                error_code = "401"
            elif "网络" in error_message or "connection" in error_message.lower():
                error_code = "network_error"

            # 1. 发送错误代码到前端（由前端翻译显示弹窗）
            error_data = {
                "type": "error",
                "error_code": error_code,
                "detail": str(e),  # 原始错误信息，用于调试
            }

            for client_id in self.config.clients:
                await message_broker.publish(client_id, error_data)

            # 2. 发送状态重置消息，让前端回到输入状态
            reset_data = {"type": "status_reset", "status": "input"}
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
