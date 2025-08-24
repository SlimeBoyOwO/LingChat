import json
import copy
from typing import List, Dict
import asyncio

from ling_chat.core.ai_service.rag_manager import RAGManager
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.core.ai_service.ai_logger import AILogger
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.logger import logger, TermColors
from ling_chat.core.ai_service.message_generator import MessageGenerator
from ling_chat.utils.function import Function

import os

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
        self.message_generator = MessageGenerator(self.voice_maker,
                                                  self.message_processor,
                                                  self.translator,
                                                  self.llm_model,
                                                  self.rag_manager,
                                                  self.ai_logger)

        self.import_settings(settings)
        self.reset_memory()

        # TODO: 本代码测试，开机后十秒提醒用户久坐
        self.schedule_times = ["22:29", "22:30"]  # TODO：默认时间表，午夜凶灵哦
        self.proceed_next_nodification()
    
    def import_settings(self, settings: dict):
        if(settings):
            self.ai_name = settings.get("ai_name","ai_name未设定")
            self.ai_subtitle = settings.get("ai_subtitle","ai_subtitle未设定")
            self.user_name = settings.get("user_name", "user_name未设定")
            self.user_subtitle = settings.get("user_subtitle", "user_subtitle未设定")
            self.ai_prompt = settings.get("system_prompt", "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息")
            self.ai_prompt_example = settings.get("system_prompt_example","")
            self.ai_prompt = self.message_processor.sys_prompt_builder(self.ai_prompt, self.ai_prompt_example)

            # 语音生成相关 TODO @影空，这里记得改一下，代码不太优雅

            self.voice_maker.set_speark_id(int(settings.get("speaker_id", 4)))
            self.voice_maker.set_model_name(settings.get("model_name", ""))
            # 假如有API，优先级更高
            if(settings.get("sbv2api_model_name", "") != ""):
                self.voice_maker.set_model_name(settings.get("sbv2api_model_name", ""))
                
            self.voice_maker.set_lang(settings.get("language", "ja"))
            self.voice_maker.set_tts_type(settings.get("tts_type", "sbv"))

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
        self.memory = [
            {
                "role": "system", 
                "content": self.ai_prompt
            }
        ]
    
    async def process_message(self, user_message: str):
        """
        对接函数：调用MessageGenerator的process_message方法处理用户消息
        """
        self.message_generator.memory_init(self.memory)
        await self.message_generator.process_message(user_message)

    async def process_message_stream_compat(self, user_message: str):
        """
        对接函数：与process_message兼容的流式消息处理函数
        这个函数收集所有的流式响应并返回一个列表
        """
        self.message_generator.memory_init(self.memory)
        logger.info("正在使用流式回复")
        responses = []
        async for response in self.message_generator.process_message_stream(user_message):
            responses.append(response)
        return responses
    
    def proceed_next_nodification(self):
        self.schedule_task.cancel()
        self.schedule_task = asyncio.create_task(self.send_nodification_by_schedule())
        
    async def send_nodification_by_schedule(self):
        """定义好的函数，在特定时间发送提醒用户不要久坐"""
        seconds:float = Function.calculate_time_to_next_reminder(self.schedule_times)
        logger.info("距离下一次提醒还有"+Function.format_seconds(seconds))
        await asyncio.sleep(seconds)
        user_message:str = "{时间差不多到啦，关心提醒一下" + self.user_name + "不要久坐吧！}"
        await self.process_message_stream_compat(user_message)
        self.proceed_next_nodification()

    async def cleanup(self):
        """简单的清理方法"""
        if hasattr(self, 'schedule_task') and self.schedule_task:
            self.schedule_task.cancel()

    