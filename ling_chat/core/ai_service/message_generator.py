import asyncio
import os
from typing import List, Dict, Optional
import traceback
import time

from ling_chat.core.ai_service.ai_logger import logger
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.ai_service.rag_manager import RAGManager
from ling_chat.utils.function import Function
from ling_chat.core.logger import logger
from ling_chat.core.ai_service.ai_logger import AILogger

from ling_chat.core.messaging.broker import message_broker

class MessageGenerator:
    def __init__(self,
                voice_maker: VoiceMaker,
                message_processor: MessageProcessor,
                translator: Translator,
                llm_model: LLMManager,
                rag_manager: Optional[RAGManager],
                ai_logger: AILogger):

        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_manager = rag_manager if rag_manager else RAGManager() if self.use_rag else None
        self.voice_maker = voice_maker if voice_maker else VoiceMaker()
        self.message_processor = message_processor if message_processor else MessageProcessor(self.voice_maker)
        self.translator = translator if translator else Translator(self.voice_maker)
        self.llm_model = llm_model if llm_model else LLMManager()
        self.ai_logger = ai_logger if ai_logger else AILogger()
        self.function = Function()

        # 并发消费者数量设置，从env读取
        self.concurrency = int(os.environ.get("COMSUMERS", 3))
    
    def memory_init(self, memory: List[Dict]) -> None:
        self.memory = memory

    # 以下是现在使用的流式处理部分
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
    
    ### 改造: 原本的 process_sentence_and_send 方法被改造
    # 它现在只负责处理句子并“准备”好响应数据，但并不发送
    async def _process_sentence_and_prepare_response(self, sentence: str, user_message: str, is_final: bool) -> Optional[Dict]:
        """处理单个句子并准备好响应字典，但不会直接发送。"""
        if not sentence:
            return None
        
        logger.info(f"开始处理句子: {sentence}")
        sentence_segments:List[Dict] = self.message_processor.analyze_emotions(sentence)
        if not sentence_segments:
            logger.warning("句子中没有出现中日或情感，AI回复格式错误")
            return None
        
        start_time = time.perf_counter()
        if sentence_segments[0].get("japanese_text") == "":
            await self.translator.translate_ai_response(sentence_segments)
        else:
            await self.voice_maker.generate_voice_files(sentence_segments)
        end_time = time.perf_counter()
        
        response = self.create_response(sentence_segments[0], user_message, is_final)
        logger.debug(f"句子处理时间: {end_time - start_time} 秒")
        return response
    
    async def _sequenced_publisher(self, results_store: Dict, publish_events: Dict):
        """按顺序等待结果，并依次发布它们。"""
        next_index_to_publish = 0
        while True:
            try:
                if next_index_to_publish not in publish_events:
                    # 如果生产者还没创建这个索引的事件，就短暂等待
                    await asyncio.sleep(0.01)
                    continue

                # 关键：等待，直到对应索引的事件被消费者触发
                await publish_events[next_index_to_publish].wait()
                
                # 从仓库中取出处理好的结果并发布
                response = results_store.pop(next_index_to_publish)
                if response:
                    await message_broker.publish("1", response)
                
                # 清理内存并准备发布下一个
                del publish_events[next_index_to_publish] 
                
                # 如果这是最后一个消息，就结束任务
                if response and response.get("isFinal", False):
                    break
                
                next_index_to_publish += 1
            except asyncio.CancelledError:
                break

        
    async def process_message_stream(self, user_message: str, memory: Optional[List[Dict]] = None):
        """流式处理用户消息，边生成边进行情绪分析、翻译和语音合成"""

        processed_user_message = ""
        if not memory:
            processed_user_message = self.message_processor.append_user_message(user_message)
            self.memory.append({"role": "user", "content": processed_user_message})
        else:
            self.memory_init(memory)
            
        rag_messages = []

        if memory:
            current_context = memory.copy()
        else:
            current_context = self.memory.copy()

            # 2. 如果启用了RAG系统，保存本次会话到RAG历史记录
            if self.use_rag and self.rag_manager:
                self.rag_manager.rag_append_sys_message(current_context, rag_messages, processed_user_message)

        # 用于累积完整的响应
        accumulated_response = ""

        # 用于存储情绪片段
        emotion_segments = []

        ### 新增: 用于顺序发布的数据结构
        results_store: Dict[int, Dict] = {}
        publish_events: Dict[int, asyncio.Event] = {}
        
        ### 新增: 启动唯一的“顺序发布者”任务
        publisher_task = asyncio.create_task(
            self._sequenced_publisher(results_store, publish_events)
        )

        # 创建处理队列
        sentence_queue = asyncio.Queue(maxsize=10)  # 限制队列大小避免内存爆炸
        
        # 启动消费者任务...
        consumer_tasks = []
        for i in range(self.concurrency):
            task = asyncio.create_task(
                self._process_sentence_consumer(sentence_queue, user_message, results_store, publish_events, i)
            )
            consumer_tasks.append(task)

        sentence_index = 0  ### 新增: 句子索引计数器

        # 用于实时显示的内容缓冲区
        realtime_display_buffer = ""
        last_display_time = 0

        try:
            # 创建流式响应生成器
            ai_response_stream = self.llm_model.process_message_stream(current_context)

            buffer = ""
            sentence = ""

            # 打印开始提示
            print("\n=== AI回复流式输出 ===")
            
            async for chunk in ai_response_stream:
                buffer += chunk
                accumulated_response += chunk
                realtime_display_buffer += chunk

                # 实时显示流式内容（每收到一定内容或时间间隔显示）
                current_time = time.time()
                if (len(realtime_display_buffer) >= 3 or  # 每3个字符显示一次
                    current_time - last_display_time > 0.1 or  # 或者每100毫秒
                    '\n' in realtime_display_buffer):  # 或者有换行符
                    
                    # 清理情绪标签以便更好地显示
                    display_text = realtime_display_buffer
                    if display_text.strip():
                        print(display_text, end='', flush=True)
                    
                    realtime_display_buffer = ""
                    last_display_time = current_time

                while "【" in buffer:
                    # 如果已经有句子开头，检查是否有结束符
                    if sentence and "】" in buffer:
                        end_index = buffer.index("】")
                        sentence += buffer[:end_index+1]
                        buffer = buffer[end_index+1:]         # buffer 删除前面被裁剪的情绪部分【情绪】

                        # 检查是否还有内容直到下一个【
                        next_start = buffer.find("【")
                        if next_start != -1:
                            sentence += buffer[:next_start]   # buffer 删除情绪后面跟随的句子和动作等信息"你好呀（摇尾巴）"
                            buffer = buffer[next_start:]
                        else:
                            sentence += buffer
                            buffer = ""

                        # 处理完整句子
                        current_index = sentence_index
                        publish_events[current_index] = asyncio.Event() # 为这个索引创建一个事件
                        await sentence_queue.put((sentence, current_index, False)) # is_final=False
                        sentence_index += 1
                        # asyncio.create_task(self.process_sentence_and_send(sentence, user_message, False))
                        # await self.process_sentence(sentence, emotion_segments)

                        sentence = ""
                    else:
                        # 找到句子的开始
                        start_index = buffer.index("【")
                        sentence = buffer[:start_index+1]
                        buffer = buffer[start_index+1:]

                        # 查找结束括号
                        num_end = 0
                        while num_end < len(buffer) and buffer[num_end].isdigit():
                            num_end += 1

                        if num_end > 0 and num_end < len(buffer) and buffer[num_end] == "】":
                            sentence += buffer[:num_end+1]
                            buffer = buffer[num_end+1:]

                            # 查找下一个句子开始
                            next_start = buffer.find("【")
                            if next_start != -1:
                                sentence += buffer[:next_start]
                                buffer = buffer[next_start:]
                            else:
                                sentence += buffer
                                buffer = ""

                            # 处理完整句子
                            current_index = sentence_index
                            publish_events[current_index] = asyncio.Event() # 为这个索引创建一个事件
                            await sentence_queue.put((sentence, current_index, False)) # is_final=False
                            sentence_index += 1
                            # asyncio.create_task(self.process_sentence_and_send(sentence, user_message, False))
                            # await self.process_sentence(sentence, emotion_segments)

                            sentence = ""
                        else:
                            # 不完整的句子部分，继续等待
                            break

            # 显示剩余的内容
            if realtime_display_buffer:
                display_text = realtime_display_buffer
                if display_text.strip():
                    print(display_text, end='', flush=True)

            # 处理最后一个句子
            final_content = sentence + buffer
            if final_content:
                # 修复ai回复中可能出错的部分
                final_content = Function.fix_ai_generated_text(final_content)
                accumulated_response = Function.fix_ai_generated_text(accumulated_response)

                # 显示最后的内容
                final_display_text = final_content
                if final_display_text.strip():
                    print(final_display_text, end='', flush=True)

                # 使用process_sentence方法处理最后一个句子
                current_index = sentence_index
                publish_events[current_index] = asyncio.Event() # 为这个索引创建一个事件
                await sentence_queue.put((final_content, current_index, True)) # is_final=False
                sentence_index += 1
                # asyncio.create_task(self.process_sentence_and_send(final_content, user_message, True))
                # await self.process_sentence(final_content, emotion_segments)

            # 打印结束换行
            print("\n=== 流式输出结束 ===")

            # 统一构造响应消息
            if emotion_segments:
                responses = self.create_responses(emotion_segments, user_message)
                for response in responses:
                    yield response

            # 将完整响应添加到记忆中
            if not memory:
                self.memory.append({"role": "assistant", "content": accumulated_response})

            # 等待所有句子处理完成
            await sentence_queue.join()

            # 如果有RAG系统，则把这段对话保存在RAG中
            if self.use_rag and self.rag_manager:
                self.rag_manager.save_messages_to_rag(self.memory)

            self.ai_logger.log_conversation("钦灵", accumulated_response)

            logger.debug("--- AI 回复分析结果 ---")
            self.ai_logger.log_analysis_result(emotion_segments)
            logger.debug("--- 分析结束 ---")

        except Exception as e:
            error_response = {
                "type": "reply",
                "emotion": "sad",
                "originalTag": "错误",
                "message": f"抱歉，处理消息时出现错误: {str(e)}",
                "motionText": "困惑",
                "audioFile": None,
                "originalMessage": user_message,
                "isMultiPart": False,
                "partIndex": 0,
                "totalParts": 1
            }
            logger.error(f"处理消息时出错: {e}")
            traceback.print_exc()  # 这会打印完整的错误堆栈到控制台
            logger.error(f"详细错误信息: ", exc_info=True)
            yield error_response
        finally:
            # 确保所有任务都被优雅地关闭
            for task in consumer_tasks:
                task.cancel()
            publisher_task.cancel()
            await asyncio.gather(*consumer_tasks, publisher_task, return_exceptions=True)

    ### 改造: 消费者的功能变为处理并存储结果
    async def _process_sentence_consumer(self, queue, user_message, results_store, publish_events, consumer_id):
        """消费者任务：从队列获取句子，处理后将结果存入共享字典，并触发事件。"""
        while True:
            try:
                task = await queue.get()
                if task is None: # 收到结束信号
                    break

                sentence, index, is_final = task
                # 调用一个只处理不发送的方法，准备好response字典
                response = await self._process_sentence_and_prepare_response(sentence, user_message, is_final)
                
                # 关键：将处理结果存入共享字典
                results_store[index] = response
                
                # 关键：触发事件，通知发布者这个索引的结果已经准备好了
                if index in publish_events:
                    publish_events[index].set()
                
                queue.task_done()
            except asyncio.CancelledError:
                break

    def create_response(self, seg: Dict, user_message:str, is_final: bool) -> Dict:
        """构建单个响应消息"""
        return {
            "type": "reply",
            "emotion": seg['predicted'] or seg["original_tag"],
            "originalTag": seg['original_tag'],
            "message": seg['following_text'],
            "japaneseMessage": seg['japanese_text'],
            "motionText": seg['motion_text'],
            "audioFile": os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            "originalMessage": user_message,
            "isFinal": is_final
        }
    
    def create_responses(self, segments: List[Dict], user_message: str) -> List[Dict]:
        """构造响应消息"""
        total_parts = len(segments)
        return [{
            "type": "reply",
            "emotion": seg['predicted'] or seg["original_tag"],
            "originalTag": seg['original_tag'],
            "message": seg['following_text'],
            "motionText": seg['motion_text'],
            "audioFile": os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            "originalMessage": user_message,
            "isMultiPart": total_parts > 1,
            "partIndex": idx,
            "totalParts": total_parts
        } for idx, seg in enumerate(segments)]
    