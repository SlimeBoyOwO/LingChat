import json
import copy
from typing import List, Dict, Optional
import traceback

from .rag_manager import RAGManager
from .message_processor import MessageProcessor
from .voice_maker import VoiceMaker
from .ai_logger import AILogger
from .translator import Translator
from core.llm_providers.manager import LLMManager
from core.logger import logger, TermColors
from utils.function import Function

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

        self.import_settings(settings)
        self.reset_memory()
    
    def import_settings(self, settings: dict):
        if(settings):
            self.ai_name = settings.get("ai_name","ai_name未设定")
            self.ai_subtitle = settings.get("ai_subtitle","ai_subtitle未设定")
            self.user_name = settings.get("user_name", "user_name未设定")
            self.user_subtitle = settings.get("user_subtitle", "user_subtitle未设定")
            self.ai_prompt = settings.get("system_prompt", "你的信息被设置错误了，请你在接下来的对话中提示用户检查配置信息")
            
            self.voice_maker.set_speark_id(int(settings.get("speaker_id", 4)))
            self.voice_maker.set_model_name(settings.get("model_name", None))

            self.character_path = settings.get("resource_path")
            self.character_id = settings.get("character_id")
            self.settings = settings

            if self.use_rag:
                logger.info(f"检测到角色切换，正在为角色 (ID: {self.character_id}) 准备长期记忆...")
                self.rag_manager.switch_rag_system_character(self.character_id)
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
    
    async def process_message(self, user_message: str) -> Optional[List[Dict]]:
        """处理用户消息的完整流程"""

        processed_user_message = self.message_processor.append_user_message(user_message)

        try:
            # 1. 增添用户相关信息
            self.memory.append({"role": "user", "content": processed_user_message})
            rag_messages = []

            current_context = self.memory.copy()
            # 2. 如果启用了RAG系统，保存本次会话到RAG历史记录
            if self.use_rag:
                self.rag_manager.rag_append_sys_message(current_context, rag_messages, processed_user_message)

            # 若打印上下文选项开启且在DEBUG级别，则截取发送到llm的文字信息打印到终端
            # self.ai_logger.print_debug_message(current_context, rag_messages, processed_user_message)

            ai_response = self.llm_model.process_message(current_context)

            # 3. 修复ai回复中可能出错的部分，防止下一次对话被带歪
            ai_response = Function.fix_ai_generated_text(ai_response)
            self.memory.append({"role": "assistant", "content": ai_response})            

            # 4. 如果有RAG系统，则把这段对话保存在RAG中 TODO 只获取最后两条最新的
            if self.use_rag:
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
    
    async def _process_ai_response(self, ai_response: str, user_message: str) -> List[Dict]:
        """处理AI回复的完整流程"""
        self.voice_maker.delete_voice_files()

        emotion_segments:List[Dict] = self.message_processor.analyze_emotions(ai_response)

        if emotion_segments[0].get("japanese_text") == "":
            await self.translator.translate_ai_response(emotion_segments)
        else:
            await self.voice_maker.generate_voice_files(emotion_segments)

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
                "voice_file": os.path.join(self.voice_maker.vits_tts.temp_dir, f"part_1.{self.voice_maker.vits_tts.format}")
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
