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

from ling_chat.utils.runtime_path import temp_path

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
    
    def memory_init(self, memory: List[Dict]) -> None:
        self.memory = memory

    
    # 兼容性：古老的非流式生成函数部分
    async def process_message(self, user_message: str) -> Optional[List[Dict]]:
        """处理用户消息的完整流程"""

        processed_user_message = self.message_processor.append_user_message(user_message)
        
        try:
            # 1. 增添用户相关信息
            self.memory.append({"role": "user", "content": processed_user_message})
            rag_messages = []

            current_context = self.memory.copy()
            # 2. 如果启用了RAG系统，保存本次会话到RAG历史记录
            if self.use_rag and self.rag_manager:
                self.rag_manager.rag_append_sys_message(current_context, rag_messages, processed_user_message)

            # 若打印上下文选项开启且在DEBUG级别，则截取发送到llm的文字信息打印到终端
            # self.ai_logger.print_debug_message(current_context, rag_messages, processed_user_message)

            # 启动一个计时器
            start_time = time.perf_counter()
            ai_response = self.llm_model.process_message(current_context)
            end_time = time.perf_counter()
            logger.debug(f"LLM输出时间: {end_time - start_time} 秒")

            # 3. 修复ai回复中可能出错的部分，防止下一次对话被带歪
            ai_response = Function.fix_ai_generated_text(ai_response)
            self.memory.append({"role": "assistant", "content": ai_response})            

            # 4. 如果有RAG系统，则把这段对话保存在RAG中 TODO 只获取最后两条最新的
            if self.use_rag and self.rag_manager:
                self.rag_manager.save_messages_to_rag(self.memory)

            self.ai_logger.log_conversation("用户", processed_user_message)
            self.ai_logger.log_conversation("钦灵", ai_response)
            
            # 5. 分析情绪和生成语音
            final_response = await self.process_ai_response(ai_response, user_message)

            error_response = [{
                "type": "reply",
                "emotion": "sad",
                "originalTag": "错误",
                "message": f"抱歉，处理消息时出现错误，回复是空的",
                "motionText": "困惑",
                "audioFile": None,
                "originalMessage": user_message,
                "isMultiPart": False,
                "partIndex": 0,
                "totalParts": 1
            }]

            if final_response is None:
                logger.error("AI服务返回了None响应")
                await message_broker.publish("1", error_response[0])
                return error_response
            else: 
                for response in final_response:
                    await message_broker.publish("1", response)
                return final_response
                
        except Exception as e:
            error_response = [{
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
            }]
            logger.error(f"处理消息时出错: {e}")
            traceback.print_exc()  # 这会打印完整的错误堆栈到控制台
            logger.error(f"详细错误信息: ", exc_info=True)
            return error_response
    
    async def process_ai_response(self, ai_response: str, user_message: str) -> List[Dict]:
        """处理AI回复的完整流程"""
        self.voice_maker.delete_voice_files()

        emotion_segments:List[Dict] = self.message_processor.analyze_emotions(ai_response)

        start_time = time.perf_counter()
        if self.voice_maker.lang == "ja":
            if emotion_segments[0].get("japanese_text") == "":
                await self.translator.translate_ai_response(emotion_segments)
            else:
                await self.voice_maker.generate_voice_files(emotion_segments)
            end_time = time.perf_counter()
            logger.debug(f"日语合成时间: {end_time - start_time} 秒")
        elif self.voice_maker.lang == "zh":
            await self.voice_maker.generate_voice_files(emotion_segments)
            end_time = time.perf_counter()
            logger.debug(f"中文合成时间: {end_time - start_time} 秒")


        if not emotion_segments:
            logger.warning("未检测到有效情绪片段")
            emotion_segments = [{
                "index": 1,
                "original_tag": "normal",
                "following_text": ai_response,
                "motion_text": "",
                "japanese_text": "",
                "predicted": "normal",
                "confidence": 0.8,
                "voice_file": str(temp_path / f"audio/part_1.{self.voice_maker.vits_tts.format}")
            }]
        
        responses = self.create_responses(emotion_segments, user_message)
    
        logger.debug("--- AI 回复分析结果 ---")
        self.ai_logger.log_analysis_result(emotion_segments)
        logger.debug("--- 分析结束 ---")

        return responses

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
            # 为每个片段生成语音和翻译

            logger.debug(f"句子处理时间: {end_time - start_time} 秒")
    
    # 句子处理完毕后，立马发送给前端的新process_sentence (流式)
    async def process_sentence_and_send(self, sentence: str, user_messsage: str, is_final: bool):
        """处理单个句子的情绪分析、翻译和语音合成，并发送给前端"""
        if not sentence:
            return
        
        logger.info("开始处理句子" + sentence)
        sentence_segments:List[Dict] = self.message_processor.analyze_emotions(sentence)
        if not sentence_segments:
            logger.warning("句子中没有出现中日或情感，AI回复格式错误")
            return
        else:
            # 翻译句子 TODO 假如翻译句子用的是比较贵的AI，这里不应该每个句子都单独飞过去翻译
            # 为每个片段生成语音和翻译
            start_time = time.perf_counter()
            if sentence_segments[0].get("japanese_text") == "":
                await self.translator.translate_ai_response(sentence_segments)
            else:
                await self.voice_maker.generate_voice_files(sentence_segments)
            end_time = time.perf_counter()
            
            # 当句子准备完毕后，发布这部分的消息
            response = self.create_response(sentence_segments[0], user_messsage, is_final)
            await message_broker.publish("1", response)

            logger.debug(f"句子处理时间: {end_time - start_time} 秒")
        logger.info("句子" + sentence + "处理完毕并发送")
        
    async def process_message_stream(self, user_message: str):
        """流式处理用户消息，边生成边进行情绪分析、翻译和语音合成"""

        processed_user_message = self.message_processor.append_user_message(user_message)

        # 1. 增添用户相关信息
        self.memory.append({"role": "user", "content": processed_user_message})
        rag_messages = []

        current_context = self.memory.copy()
        # 2. 如果启用了RAG系统，保存本次会话到RAG历史记录
        if self.use_rag and self.rag_manager:
            self.rag_manager.rag_append_sys_message(current_context, rag_messages, processed_user_message)

        # 用于累积完整的响应
        accumulated_response = ""

        # 用于存储情绪片段
        emotion_segments = []

        # 创建处理队列
        sentence_queue = asyncio.Queue(maxsize=10)  # 限制队列大小避免内存爆炸
        
        # 启动多个消费者任务（3-5个比较合适）
        consumer_tasks = []
        for i in range(3):  # 3个并行消费者
            task = asyncio.create_task(
                self._process_sentence_consumer(sentence_queue, user_message, i)
            )
            consumer_tasks.append(task)

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
                        await sentence_queue.put((sentence, False))
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
                            await sentence_queue.put((sentence, False))
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

                # 使用process_sentence方法处理句子
                await sentence_queue.put((final_content, True))
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
            # 取消所有消费者任务
            for task in consumer_tasks:
                task.cancel()
            await asyncio.gather(*consumer_tasks, return_exceptions=True)

    async def _process_sentence_consumer(self, queue, user_message, consumer_id):
        """消费者任务，从队列中取出句子处理"""
        """多个消费者并行处理句子"""
        while True:
            try:
                sentence, is_final = await queue.get()
                await self.process_sentence_and_send(sentence, user_message, is_final)
                queue.task_done()
            except asyncio.CancelledError:
                break
            except Exception as e:
                queue.task_done()

    def create_response(self, seg: Dict, user_message:str, is_final: bool) -> Dict:
        """构建单个响应消息"""
        return {
            "type": "reply",
            "emotion": seg['predicted'] or seg["original_tag"],
            "originalTag": seg['original_tag'],
            "message": seg['following_text'],
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
    
    # 以下是新的流式处理函数，采用了新的返回格式，所以暂未实装
    async def process_message_stream_new(self, user_message: str):
        """流式处理用户消息，边生成边进行情绪分析、翻译和语音合成"""

        processed_user_message = self.message_processor.append_user_message(user_message)

        # 1. 增添用户相关信息
        self.memory.append({"role": "user", "content": processed_user_message})
        rag_messages = []

        current_context = self.memory.copy()
        # 2. 如果启用了RAG系统，保存本次会话到RAG历史记录
        if self.use_rag and self.rag_manager:
            self.rag_manager.rag_append_sys_message(current_context, rag_messages, processed_user_message)

        # 用于累积完整的响应
        accumulated_response = ""

        # 用于存储情绪片段
        emotion_segments = []

        try:
            # 创建流式响应生成器
            ai_response_stream = self.llm_model.process_message_stream(current_context)

            buffer = ""
            sentence = ""

            async for chunk in ai_response_stream:
                buffer += chunk
                accumulated_response += chunk

                while "【" in buffer:
                    # 如果已经有句子开头，检查是否有结束符
                    if sentence and "】" in buffer:
                        end_index = buffer.index("】")
                        sentence += buffer[:end_index+1]
                        buffer = buffer[end_index+1:]

                        # 检查是否还有内容直到下一个【
                        next_start = buffer.find("【")
                        if next_start != -1:
                            sentence += buffer[:next_start]
                            buffer = buffer[next_start:]
                        else:
                            sentence += buffer
                            buffer = ""

                        # 处理完整句子
                        if sentence:
                            async for response in self._process_sentence_new(sentence, emotion_segments, user_message):
                                yield response

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
                            if sentence:
                                async for response in self._process_sentence_new(sentence, emotion_segments, user_message):
                                    yield response

                            sentence = ""
                        else:
                            # 不完整的句子部分，继续等待
                            break

            # 处理最后一个句子
            final_content = sentence + buffer
            if final_content:
                # 修复ai回复中可能出错的部分
                final_content = Function.fix_ai_generated_text(final_content)
                accumulated_response = Function.fix_ai_generated_text(accumulated_response)

                # 处理最后一个句子并设置 isFinal=True
                async for response in self._process_sentence_new(final_content, emotion_segments, user_message, isFinal=True):
                    yield response

            # 将完整响应添加到记忆中
            self.memory.append({"role": "assistant", "content": accumulated_response})

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
                "isFinal": True
            }
            logger.error(f"处理消息时出错: {e}")
            traceback.print_exc()  # 这会打印完整的错误堆栈到控制台
            logger.error(f"详细错误信息: ", exc_info=True)
            yield error_response

    async def _process_sentence_new(self, sentence: str, emotion_segments: List[Dict], user_message: str, isFinal: bool = False):
        """处理完整的句子，包括情绪分析、语音/翻译生成和响应构造"""
        sentence_segments = self.message_processor.analyze_emotions(sentence)
        if sentence_segments:
            # 更新情绪片段列表
            emotion_segments.extend(sentence_segments)

            # 为每个片段生成语音和翻译
            start_time = time.perf_counter()
            if sentence_segments[0].get("japanese_text") == "":
                await self.translator.translate_ai_response(sentence_segments)
            else:
                await self.voice_maker.generate_voice_files(sentence_segments)
            end_time = time.perf_counter()
            logger.debug(f"句子处理时间: {end_time - start_time} 秒")

            # 立即构造并返回响应
            responses = self.create_responses_new(sentence_segments, user_message, isFinal)
            for response in responses:
                yield response

    def create_responses_new(self, segments: List[Dict], user_message: str, isFinal: bool = False) -> List[Dict]:
        """构造响应消息"""
        responses = [{
            "type": "reply",
            "emotion": seg['predicted'] or seg["original_tag"],
            "originalTag": seg['original_tag'],
            "message": seg['following_text'],
            "motionText": seg['motion_text'],
            "audioFile": os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            "originalMessage": user_message,
            "isFinal": isFinal
        } for idx, seg in enumerate(segments)]

        return responses
