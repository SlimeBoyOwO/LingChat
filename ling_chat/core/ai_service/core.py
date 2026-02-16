import asyncio
import copy
import json
import os
from typing import Dict

from ling_chat.core.ai_service.exceptions import ScriptEngineError
from ling_chat.core.ai_service.game_system.game_status import GameStatus
from ling_chat.game_database.models import GameLine, LineAttribute, LineBase
from ling_chat.core.ai_service.ai_logger import AILogger
from ling_chat.core.ai_service.config import AIServiceConfig
from ling_chat.core.ai_service.events_scheduler import EventsScheduler
from ling_chat.core.ai_service.message_system.message_processor import MessageProcessor
from ling_chat.core.ai_service.message_system.message_generator import MessageGenerator
from ling_chat.core.ai_service.script_engine.script_manager import ScriptManager
from ling_chat.core.ai_service.translator import Translator
from ling_chat.core.llm_providers.manager import LLMManager
from ling_chat.utils.function import Function
from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker

class AIService:
    def __init__(self, settings: dict[str, str]):

        """
        初始化AI助手实例
        
        参数:
            settings: 配置字典，包含各种设置项
        """
        self.game_status = GameStatus()

        self.user_id = "1"   # TODO: 多用户的时候这里可以改成按照初始化获取，或者直接从client_id中获取

        self.config = AIServiceConfig(clients=set(), user_id=self.user_id)

        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.llm_model = LLMManager()
        self.ai_logger = AILogger()
        self.translator = Translator(self.game_status)
        self.message_broker = message_broker
        self.message_processor = MessageProcessor(self.game_status)
        self.message_generator = MessageGenerator(self.config,
                                                  self.message_processor,
                                                  self.translator,
                                                  self.llm_model,
                                                  self.ai_logger,
                                                  self.game_status)

        # self.events_scheduler.start_nodification_schedules()        # 之后会通过API设置和处理

        # self.output_queue_name = self.client_id             # WebSocket输出队列
        self.client_tasks: Dict[str, asyncio.Task] = {}
        self.processing_task = asyncio.create_task(self._process_message_loop())
        self.global_task = asyncio.create_task(self._process_global_messages())

        self.events_scheduler = EventsScheduler(self.config)
        self.import_settings(settings)
        self.events_scheduler.start_nodification_schedules()        # TODO: 这个由前端开关控制

        self.scripts_manager = ScriptManager(self.config, self.game_status)

        # 特别的，设定当游戏角色被导入的时候，设定它为游戏主角，其他情况下则以变量为准，并初始化system prompt
        self._init_game_status()

    def import_settings(self, settings: Dict) -> None:
        # TODO: 这些以后全都可以删除，改为通过修改game_status的GameRole来实现
        if(settings):
            self.character_path = settings.resource_path
            self.character_id = settings.character_id  # TODO: character_id就是查找的role_id，这里写的不太优雅，可以之后优化
            self.ai_name = settings.ai_name
            self.ai_subtitle = settings.ai_subtitle
            self.user_name = settings.user_name
            self.user_subtitle = settings.user_subtitle
            self.ai_prompt = settings.system_prompt or "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息"
            self.game_status.player.user_name = self.user_name
            self.game_status.player.user_subtitle = self.user_subtitle

            self.ai_prompt_example = settings.system_prompt_example
            self.ai_prompt_example_old = settings.system_prompt_example_old
            self.ai_prompt = Function.sys_prompt_builder(self.user_name,
                                                         self.ai_name,
                                                         self.ai_prompt,
                                                         self.ai_prompt_example,
                                                         self.ai_prompt_example_old
                                                         )

            self.clothes_name = settings.clothes_name
            self.body_part = settings.body_part
            self.clothes = settings.clothes
            self.settings = settings

        else:
            logger.error("角色信息settings没有被正常导入，请检查问题！")

        self.events_scheduler.ai_name = self.ai_name
        self.events_scheduler.user_name = self.user_name

    def apply_runtime_config(self, updates: dict[str, str]) -> None:
        """
        根据运行时更新的配置，热重载关键组件，避免重启。
        仅在有相关键变动时做最小开销的重建。
        """
        try:
            llm_keys = {
                "LLM_PROVIDER", "MODEL_TYPE", "CHAT_API_KEY", "CHAT_BASE_URL",
                "TRANSLATE_LLM_PROVIDER", "TRANSLATE_MODEL", "TRANSLATE_API_KEY", "TRANSLATE_BASE_URL"
            }
            if any(k in updates for k in llm_keys):
                self.llm_model = LLMManager()
                self.message_generator.llm_model = self.llm_model
                logger.info("运行时配置更新：LLMManager 已重建并替换。")

            if "COMSUMERS" in updates:
                try:
                    new_concurrency = int(os.environ.get("COMSUMERS", self.message_generator.concurrency))
                    if new_concurrency > 0:
                        self.message_generator.concurrency = new_concurrency
                        logger.info(f"运行时配置更新：并发消费者数量设置为 {new_concurrency}")
                except Exception:
                    logger.warning("COMSUMERS 配置无效，忽略。")

        except Exception as e:
            logger.error(f"应用运行时配置失败: {e}", exc_info=True)

    def get_lines(self):
        return self.game_status.line_list
    
    def set_active_save_id(self, save_id: int | None):
        """
        设置当前激活存档，用于 MemoryBank 持久化/自动压缩等逻辑。
        """
        self.game_status.active_save_id = save_id

    def load_lines(self, lines:list[GameLine], main_role_id: int, save_id: int | None = None):
        self.game_status.line_list = lines
        if save_id is not None:
            self.set_active_save_id(save_id)
            # 仅在“载入存档”时从 DB 载入 MemoryBank 到运行时缓存
            involved_role_ids = set()
            for line in self.game_status.line_list:
                if line.sender_role_id:
                    involved_role_ids.add(line.sender_role_id)
                involved_role_ids.update(line.perceived_role_ids or [])
            self.game_status.role_manager.load_memory_banks_from_db(save_id, list(involved_role_ids))

        self.game_status.role_manager.sync_memories(self.game_status.line_list)
        main_role = self.game_status.role_manager.get_role(role_id=main_role_id)

        if main_role:
            # TODO: 以后加载台词的时候，current_character应该由存档中的变量数据来决定，而不是直接使用存档主角。
            self.game_status.current_character = main_role
            self.game_status.main_role = main_role
        else:
            logger.error(f"存档的主角色ID {main_role_id} 未找到。")

    def reset_lines(self):
        self._init_game_status()

    def persist_memory_banks(self, save_id: int):
        """
        将运行时的 memory_bank 缓存写入 DB（仅在创建/保存存档时调用）。
        """
        self.game_status.role_manager.persist_memory_banks_to_db(save_id)
    
    def _init_game_status(self):
        self.game_status.line_list = []
        system_line = LineBase(content=self.ai_prompt, attribute=LineAttribute.SYSTEM, sender_role_id=self.character_id, display_name=self.ai_name)
        self.game_status.add_line(system_line)
        if self.character_id:
            self.game_status.current_character = self.game_status.role_manager.get_role(self.character_id)
            self.game_status.present_roles.add(self.game_status.current_character)
            self.game_status.main_role = self.game_status.current_character
            logger.info(f"初始化游戏主角：{self.game_status.current_character} 已初始化。")
        else:
            logger.error("初始化游戏主角失败，未指定角色ID。")

    def show_lines(self):
        logger.info("当前台词列表如下：")
        for line in self.game_status.line_list:
            logger.info(f"{line.display_name} : 【{line.original_emotion}】{line.content}<{line.tts_content}>（{line.action_content}）")
    
    def show_current_role_memory(self):
        logger.info(f"当前角色的记忆列表如下：")
        logger.info(f"{self.game_status.current_character.memory}")

    async def start_script(self, script_name: str | None = None):
        """
        开始剧本模式。
        - script_name=None: 使用剧本列表第一个
        - script_name=指定: 使用指定剧本名（story_config.yaml 的 script_name）
        """
        script_list = self.scripts_manager.get_script_list()
        if not script_list:
            raise ScriptEngineError("没有可用的剧本。")

        chosen = script_name or script_list[0]
        ok = await self.scripts_manager.start_script(chosen)
        if not ok:
            raise ScriptEngineError(f"剧本 {chosen} 加载失败。")

    async def _process_client_messages(self, client_id: str):
        """处理单个客户端的消息"""
        input_queue_name = f"ai_input_{client_id}"
        try:
            async for message in self.message_broker.subscribe(input_queue_name):
                try:
                    self.is_processing = True

                    user_message = message.get("content", "")
                    if user_message:

                        responses = []
                        async for response in self.message_generator.process_message_stream(user_message=user_message):
                            await message_broker.publish(client_id, response.model_dump())
                            responses.append(response)

                        logger.debug(f"消息处理完成，共生成 {len(responses)} 个响应片段")

                    self.is_processing = False

                except Exception as e:
                    logger.error(f"处理消息时发生错误: {e}")
                    self.is_processing = False
                    # 错误通知由 message_generator 处理

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

    async def _process_global_messages(self):
        """处理全局消息"""
        global_queue_name = "ai_input_global"
        try:
            async for message in self.message_broker.subscribe(global_queue_name):
                try:
                    self.is_processing = True

                    user_message = message.get("content", "")
                    if user_message:
                        # self.message_generator.memory_init(self.memory)

                        responses = []
                        async for response in self.message_generator.process_message_stream(user_message):
                            # 发送给所有客户端
                            for client_id in self.config.clients:
                                await message_broker.publish(client_id, response.model_dump())
                            responses.append(response)

                        logger.debug(f"全局消息处理完成，共生成 {len(responses)} 个响应片段")

                    self.is_processing = False

                except Exception as e:
                    logger.error(f"处理全局消息时发生错误: {e}")
                    self.is_processing = False
                    # 错误通知由 message_generator 处理
        except asyncio.CancelledError:
            logger.info("全局消息处理任务已被取消")
        except Exception as e:
            logger.error(f"全局消息处理发生严重错误: {e}")
            raise

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

        # 取消全局消息处理任务
        self.global_task.cancel()
        try:
            await self.global_task
        except asyncio.CancelledError:
            pass

        # 取消主处理任务
        self.processing_task.cancel()
        try:
            await self.processing_task
        except asyncio.CancelledError:
            pass

        logger.info("AI服务已关闭")
