import json
import copy
from typing import List, Dict
import asyncio

from ling_chat.core.ai_service.rag_manager import RAGManager
from ling_chat.core.ai_service.message_processor import MessageProcessor
from ling_chat.core.ai_service.voice_maker import VoiceMaker
from ling_chat.core.ai_service.ai_logger import AILogger
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.ai_service.events_scheduler import EventsScheduler
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.logger import logger
from ling_chat.core.ai_service.message_system.message_generator import MessageGenerator
from ling_chat.core.ai_service.script_engine.script_manager import ScriptManager
from ling_chat.utils.function import Function

import os

class AIService:
    def __init__(self, settings: dict[str, str]):

        """
        初始化AI助手实例
        
        参数:
            settings: 配置字典，包含各种设置项
        """
        self.memory = []  # 存储对话历史记录的列表
        self.user_id = "1"   # TODO: 多用户的时候这里可以改成按照初始化获取，或者直接从client_id中获取

        self.config = AIServiceConfig(clients=set(), user_id=self.user_id)
        
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_manager = RAGManager() if self.use_rag else None
        self.llm_model = LLMManager()
        self.ai_logger = AILogger()
        self.voice_maker = VoiceMaker()
        self.translator = Translator(self.voice_maker)
        self.message_broker = message_broker
        self.message_processor = MessageProcessor(self.voice_maker)
        self.message_generator = MessageGenerator(self.config,
                                                  self.voice_maker,
                                                  self.message_processor,
                                                  self.translator,
                                                  self.llm_model,
                                                  self.rag_manager,
                                                  self.ai_logger)

        # self.events_scheduler.start_nodification_schedules()        # 之后会通过API设置和处理
        self.input_messages: list[str] = [] 

        # self.output_queue_name = self.client_id             # WebSocket输出队列
        self.client_tasks: Dict[str, asyncio.Task] = {}
        self.processing_task = asyncio.create_task(self._process_message_loop())

        self.events_scheduler = EventsScheduler(self.config)
        self.import_settings(settings)
        self.events_scheduler.start_nodification_schedules()        # TODO: 这个由前端开关控制

        self.scripts_manager = ScriptManager(self.config)

        self.reset_memory()

        
    
    def import_settings(self, settings: dict[str, str]) -> None:
        if(settings):
            self.ai_name = settings.get("ai_name","ai_name未设定")
            self.ai_subtitle = settings.get("ai_subtitle","ai_subtitle未设定")
            self.user_name = settings.get("user_name", "user_name未设定")
            self.user_subtitle = settings.get("user_subtitle", "user_subtitle未设定")
            self.ai_prompt = settings.get("system_prompt", "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息")
            self.ai_prompt_example = settings.get("system_prompt_example","")
            self.ai_prompt_example_old = settings.get("system_prompt_example_old", "")
            self.ai_prompt = self.message_processor.sys_prompt_builder(self.user_name,
                                                                       self.ai_name,
                                                                       self.ai_prompt,
                                                                       self.ai_prompt_example,
                                                                       self.ai_prompt_example_old)
                
            self.voice_maker.set_lang(settings.get("language", "ja"))
            # 设置角色路径，以便在TTS设置中使用
            self.voice_maker.set_character_path(settings.get("resource_path", ""))
            self.voice_maker.set_tts(settings.get("tts_type", "sbv"), 
                                     settings.get("voice_models", {}), 
                                     self.ai_name)

            self.character_path = settings.get("resource_path")
            self.character_id = settings.get("character_id")
            self.settings = settings

            if self.use_rag and self.rag_manager:
                logger.info(f"检测到角色切换，正在为角色 (ID: {self.character_id}) 准备长期记忆...")
                self.rag_manager.switch_rag_system_character(int(self.character_id) if self.character_id else 0)

        else:
            logger.error("角色信息settings没有被正常导入，请检查问题！")
        
        self.events_scheduler.ai_name = self.ai_name
        self.events_scheduler.user_name = self.user_name
    
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
    
    async def start_script(self):
        await self.scripts_manager.start_script()
    
    async def _process_client_messages(self, client_id: str):
        """处理单个客户端的消息"""
        input_queue_name = f"ai_input_{client_id}"
        try:
            async for message in self.message_broker.subscribe(input_queue_name):
                try:
                    self.is_processing = True
                    
                    user_message = message.get("content", "")
                    if user_message:
                        self.message_generator.memory_init(self.memory)
                        
                        responses = []
                        async for response in self.message_generator.process_message_stream(user_message):
                            await message_broker.publish(client_id, response.model_dump())
                            responses.append(response)
                        
                        logger.debug(f"消息处理完成，共生成 {len(responses)} 个响应片段")
                    
                    self.is_processing = False
                    
                except Exception as e:
                    logger.error(f"处理消息时发生错误: {e}")
                    self.is_processing = False
        except asyncio.CancelledError:
            logger.info(f"客户端 {client_id} 的消息处理任务已被取消")
        except Exception as e:
            logger.error(f"客户端 {client_id} 的消息处理发生严重错误: {e}")
            raise

    async def _process_message_loop(self):
        """主消息处理循环"""
        while True:
            try:
                # 检查是否有新的客户端需要添加
                for client_id in self.config.clients:
                    if client_id not in self.client_tasks:
                        task = asyncio.create_task(self._process_client_messages(client_id))
                        self.client_tasks[client_id] = task
                        logger.info(f"已为客户端 {client_id} 创建消息处理任务")

                # 检查是否有客户端任务已完成或需要移除
                for client_id in list(self.client_tasks.keys()):
                    if client_id not in self.config.clients:
                        task = self.client_tasks[client_id]
                        task.cancel()
                        try:
                            await task
                        except asyncio.CancelledError:
                            pass
                        del self.client_tasks[client_id]
                        logger.info(f"已移除客户端 {client_id} 的消息处理任务")

                await asyncio.sleep(0.1)  # 短暂休眠以避免过度占用CPU

            except Exception as e:
                logger.error(f"消息处理循环发生错误: {e}")
                await asyncio.sleep(1)  # 发生错误时等待较长时间再重试

    async def add_client(self, client_id: str):
        """添加新客户端"""
        logger.info(f"添加客户端: {client_id}")
        self.config.clients.add(client_id)
        # 消息处理循环会在下一次迭代时自动创建新任务

    async def remove_client(self, client_id: str):
        """移除客户端"""
        logger.info(f"移除客户端: {client_id}")
        self.config.clients.discard(client_id)
        # 消息处理循环会在下一次迭代时自动取消并清理任务

    async def shutdown(self):
        """优雅关闭服务"""
        logger.info("正在关闭AI服务...")
        
        # 取消所有客户端任务
        for client_id, task in self.client_tasks.items():
            task.cancel()
            try:
                await task
            except asyncio.CancelledError:
                pass
        
        # 取消主处理任务
        self.processing_task.cancel()
        try:
            await self.processing_task
        except asyncio.CancelledError:
            pass
        
        logger.info("AI服务已关闭")