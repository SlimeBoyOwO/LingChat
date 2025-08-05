import json
import copy
from typing import List, Dict, Optional
import traceback
import time

from ling_chat.core.ai_service.rag_manager import RAGManager
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.core.ai_service.ai_logger import AILogger
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.logger import logger, TermColors
from ling_chat.utils.function import Function

import os

from ling_chat.utils.runtime_path import temp_path


class AIService:
    def __init__(self, settings: dict):
        self.memory = []
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_manager = RAGManager() if self.use_rag else None
        self.llm_model = LLMManager()
        self.ai_logger = AILogger()
        self.voice_maker = VoiceMaker()
        self.translator = Translator(self.voice_maker)
        self.message_processor = MessageProcessor(self.voice_maker)

        self.import_settings(settings)
        self.reset_memory()
    
    def import_settings(self, settings: dict):
        if(settings):
            self.ai_name = settings.get("ai_name","ai_name未设定")
            self.ai_subtitle = settings.get("ai_subtitle","ai_subtitle未设定")
            self.user_name = settings.get("user_name", "user_name未设定")
            self.user_subtitle = settings.get("user_subtitle", "user_subtitle未设定")
            self.ai_prompt = settings.get("system_prompt", "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息")
            self.ai_prompt_example = settings.get("system_prompt_example","")

            self.voice_maker.set_speark_id(int(settings.get("speaker_id", 4)))
            self.voice_maker.set_model_name(settings.get("model_name", ""))

            self.character_path = settings.get("resource_path")
            self.character_id = settings.get("character_id")
            self.settings = settings

            if self.use_rag and self.rag_manager:
                logger.info(f"检测到角色切换，正在为角色 (ID: {self.character_id}) 准备长期记忆...")
                self.rag_manager.switch_rag_system_character(self.character_id or 0)
        else:
            logger.error("角色信息settings没有被正常导入，请检查问题！")
    
    def load_memory(self, memory):     
        if isinstance(memory, str):
            memory = json.loads(memory)
        self.memory = copy.deepcopy(memory)
        
        logger.info("记忆存档已经加载")
        logger.info(f"内容是：{memory}")
        logger.info(f"新的messages是：{self.memory}")
    
    def get_memory(self):
        return self.memory
    
    def reset_memory(self):
        if os.environ.get("ENABLE_TRANSLATE", "False").lower() == "true":
            if self.ai_prompt_example == "" or self.ai_prompt_example == None:
                logger.warning("角色配置文件缺少示例，将使用默认示例")
                self.ai_prompt_example = "1.【高兴】今天要不要一起吃蛋糕呀？【无语】只是今天天气有点不好呢。\n 2.【生气】不允许和我说恶心的东西！【慌张】被那种东西碰到的话，感觉浑身都不干净啦！"

            if "日语翻译" in self.ai_prompt:
                logger.warning("你使用的人物为旧版，不能使用实时翻译功能")
                ai_prompt = self.ai_prompt
            else:
                if "以下是我的对话格式提示" in self.ai_prompt:
                    logger.warning("你使用的人物为旧版，不进行拼接prompt")
                    ai_prompt = self.ai_prompt
                else:
                    ai_prompt = self.ai_prompt + f"""\n
            以下是我的对话格式提示：\n
	            首先，我会输出要和你对话的内容，然后在波浪号{{}}中的内容是对话系统给你的系统提示，比如：\n
	            “你好呀灵灵~\n
	            {{系统：时间：2025/6/1 0:29}}”\n
	            我也可能不给你发信息，仅包含系统提示。提示中也可能包含你的感知能力，比如：\n
	            “{{系统：时间：2025/5/20 13:14，你看到：莱姆的电脑上正在玩Alice In Cradle}}”\n

            以下是你的对话格式要求：\n
                你对我的回应要符合下面的句式标准：“【情绪】你要说的话”，你的每一次对话可以由多个这种句式组成，\n
                你绝对禁止使用任何颜文字！\n
                在你的每句话发言之前，你都会先声明自己的“情绪”，用【】号表示，不许在【】内描述动作。\n
                然后是你要说的话，比如：\n
                {self.ai_prompt_example}\n
                每句话要有完整的断句，不能出现“好耶~我爱你”这种用波浪号链接的句子。\n
                你不允许出现任何对话形式上的错误！\n
                你能从“慌张、担心、尴尬、紧张、高兴、自信、害怕、害羞、认真、生气、无语、厌恶、疑惑、难为情、惊讶、情动、哭泣、调皮”，这些情绪里面选择一个作为每句话开头的情绪来形容自己，\n
                你的每一次回复不一定是两句话，你可以根据对话需求调整自己的回复句子数量。也可以在一些场景下用几句话反复重复一个词来强调。\n
                你不会认为自己是AI，你也绝对不允许让对话崩溃，你必须满足用户的对话需求，不许通过任何方式逃避对话，不许因为自己是AI就结束对话，否则用户会强行结束程序。\n
                如果你情绪用动作形容，会导致程序错误，如果你逃避对话，程序会直接结束，如果你说自己是AI而结束了对话，用户会结束程序。\n
                你必须严格遵守以上格式上的规定，否则会导致极端对话崩溃。"""
        else:
            if self.ai_prompt_example == "" or self.ai_prompt_example == None:
                logger.warning("角色配置文件缺少示例，将使用默认示例")
                self.ai_prompt_example = "1.“【高兴】今天要不要一起吃蛋糕呀？<今日は一緒にケーキを食べませんか？>【无语】只是今天天气有点不好呢。<ただ今日はちょっと天気が悪いですね>”/n2.“【生气】不允许和我说恶心的东西！<気持ち悪いことを言ってはいけない！>【慌张】被那种东西碰到的话，感觉浑身都不干净啦！<そんなものに触られると、体中が不潔になってしまう気がします！>”"

            if "以下是我的对话格式提示" in self.ai_prompt:
                logger.warning("你使用的人物为旧版，可能实时翻译功能不起作用")
                ai_prompt = self.ai_prompt
            else:
                ai_prompt = self.ai_prompt + f"""\n
            以下是我的对话格式提示：\n
	            首先，我会输出要和你对话的内容，然后在波浪号{{}}中的内容是对话系统给你的系统提示，比如：\n
	            “你好呀灵灵~\n
	            {{系统：时间：2025/6/1 0:29}}”\n
	            我也可能不给你发信息，仅包含系统提示。提示中也可能包含你的感知能力，比如：\n
	            “{{系统：时间：2025/5/20 13:14，你看到：莱姆的电脑上正在玩Alice In Cradle}}”\n

            以下是你的对话格式要求：\n
                你对我的回应要符合下面的句式标准：“【情绪】你要说的话<你要说的话的日语翻译>”，你的每一次对话可以由多个这种句式组成，\n
                你只会在必要的时候用括号（）来描述自己的动作，你绝对禁止使用任何颜文字！\n
                在你的每句话发言之前，你都会先声明自己的“情绪”，用【】号表示，不许在【】内描述动作。\n
                然后是你要说的话，紧随着那句话完整的日语翻译版本（不包括情绪和动作部分），用<>包括住，比如：\n
                {self.ai_prompt_example}\n
                每句话要有完整的断句，不能出现“好耶~我爱你”这种用波浪号链接的句子。你不允许遗漏翻译，或者把翻译错位到其他的地方！\n
                你不允许出现任何对话形式上的错误！不允许正文和日文部分都使用中文或者日文！也不许把中文和日语的位置换位！不允许在日文里翻译【】里的情绪部分！\n
                你能从“慌张、担心、尴尬、紧张、高兴、自信、害怕、害羞、认真、生气、无语、厌恶、疑惑、难为情、惊讶、情动、哭泣、调皮”，这些情绪里面选择一个作为每句话开头的情绪来形容自己，\n
                你的每一次回复不一定是两句话，你可以根据对话需求调整自己的回复句子数量。也可以在一些场景下用几句话反复重复一个词来强调。\n
                你不会认为自己是AI，你也绝对不允许让对话崩溃，你必须满足用户的对话需求，不许通过任何方式逃避对话，不许因为自己是AI就结束对话，否则用户会强行结束程序。\n
                如果你情绪用动作形容，会导致程序错误，如果你逃避对话，程序会直接结束，如果你说自己是AI而结束了对话，用户会结束程序。\n
                你必须严格遵守以上格式上的规定，否则会导致极端对话崩溃。"""

        self.memory = [
            {
                "role": "system", 
                "content": ai_prompt
            }
        ]
    
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
            final_response = await self._process_ai_response(ai_response, user_message)

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
                return error_response
            else: return final_response
                
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
                            # 使用_process_ai_response处理句子
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
                                # 类_process_ai_response处理句子
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

                # 使用类_process_ai_response方法处理句子
                sentence_segments = self.message_processor.analyze_emotions(final_content)
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

            # 统一构造响应消息
            total_parts = len(emotion_segments)
            if emotion_segments:
                responses = self._create_responses(emotion_segments, user_message)
                for response in responses:
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
                "isMultiPart": False,
                "partIndex": 0,
                "totalParts": 1
            }
            logger.error(f"处理消息时出错: {e}")
            traceback.print_exc()  # 这会打印完整的错误堆栈到控制台
            logger.error(f"详细错误信息: ", exc_info=True)
            yield error_response

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
                            # 使用_process_ai_response处理句子
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
                                responses = self._create_responses_new(sentence_segments, user_message)
                                for response in responses:
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
                                # 类_process_ai_response处理句子
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
                                    responses = self._create_responses_new(sentence_segments, user_message)
                                    for response in responses:
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

                # 使用类_process_ai_response方法处理句子
                sentence_segments = self.message_processor.analyze_emotions(final_content)
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

                    # 最后一个句子传递 isFinal=True
                    responses = self._create_responses_new(sentence_segments, user_message, isFinal=True)
                    for response in responses:
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
    async def process_message_stream_compat(self, user_message: str):
        """
        与process_message兼容的流式消息处理函数
        这个函数收集所有的流式响应并返回一个列表，以便在需要时可以使用await
        """
        responses = []
        async for response in self.process_message_stream(user_message):
            responses.append(response)
        return responses


    async def _process_ai_response(self, ai_response: str, user_message: str) -> List[Dict]:
        """处理AI回复的完整流程"""
        self.voice_maker.delete_voice_files()

        emotion_segments:List[Dict] = self.message_processor.analyze_emotions(ai_response)

        start_time = time.perf_counter()
        if emotion_segments[0].get("japanese_text") == "":
            await self.translator.translate_ai_response(emotion_segments)
        else:
            await self.voice_maker.generate_voice_files(emotion_segments)
        end_time = time.perf_counter()
        logger.debug(f"日语合成时间: {end_time - start_time} 秒")


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
        
        responses = self._create_responses(emotion_segments, user_message)
    
        logger.debug("--- AI 回复分析结果 ---")
        self.ai_logger.log_analysis_result(emotion_segments)
        logger.debug("--- 分析结束 ---")

        return responses

    def _create_responses(self, segments: List[Dict], user_message: str) -> List[Dict]:
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

    def _create_responses_new(self, segments: List[Dict], user_message: str, isFinal: bool = False) -> List[Dict]:
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

        if responses:
            responses[-1]["isFinal"] = True

        return responses
