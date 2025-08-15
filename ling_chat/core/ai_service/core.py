import json
import copy
from typing import List, Dict

from ling_chat.core.ai_service.rag_manager import RAGManager
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.core.ai_service.ai_logger import AILogger
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.logger import logger, TermColors
from ling_chat.core.ai_service.message_generator import MessageGenerator

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
    
    def import_settings(self, settings: dict):
        if(settings):
            self.ai_name = settings.get("ai_name","ai_name未设定")
            self.ai_subtitle = settings.get("ai_subtitle","ai_subtitle未设定")
            self.user_name = settings.get("user_name", "user_name未设定")
            self.user_subtitle = settings.get("user_subtitle", "user_subtitle未设定")
            self.ai_prompt = settings.get("system_prompt", "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息")
            self.ai_prompt_example = settings.get("system_prompt_example","")
            self.ai_prompt = self.message_processor.sys_prompt_builder(self.ai_prompt, self.ai_prompt_example)

            # 语音生成相关
            if settings.get("sbv2api_model_name") != (None or ""):
                self.voice_maker.set_model_name(settings.get("sbv2api_model_name", ""))
            else:
                self.voice_maker.set_model_name(settings.get("model_name", ""))
                self.voice_maker.set_speark_id(int(settings.get("speaker_id", 4)))

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
    
    async def process_message(self, user_message: str) -> List[Dict]:
        """
        对接函数：调用MessageGenerator的process_message方法处理用户消息
        """
        self.message_generator.memory_init(self.memory)
        return await self.message_generator.process_message(user_message)

    async def process_message_stream_compat(self, user_message: str):
        """
        对接函数：与process_message兼容的流式消息处理函数
        这个函数收集所有的流式响应并返回一个列表
        """
        self.message_generator.memory_init(self.memory)
        responses = []
        async for response in self.message_generator.process_message_stream(user_message):
            responses.append(response)
        return responses