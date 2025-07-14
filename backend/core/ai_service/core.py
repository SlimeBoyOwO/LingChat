from typing import List, Dict, Optional
import traceback

from .rag_manager import RAGManager
from .message_processor import MessageProcessor
from .voice_maker import VoiceMaker
from core.llm_providers.manager import LLMManager
from core.logger import logger, TermColors
from core.dialog_logger import DialogLogger
from utils.function import Function

import os

class AIService:
    def __init__(self, settings: dict):
        self.memory = []
        self.use_rag = os.environ.get("USE_RAG", "False").lower() == "true"
        self.rag_manager = RAGManager() if self.use_rag else None
        self.llm_model = LLMManager()
        self.dialog_logger = DialogLogger()
        self.voice_maker = VoiceMaker()
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
            # self._print_debug_message(current_context, rag_messages, processed_user_message)

            ai_response = self.llm_model.process_message(current_context)

            # 3. 修复ai回复中可能出错的部分，防止下一次对话被带歪
            ai_response = Function.fix_ai_generated_text(ai_response)
            self.memory.append({"role": "assistant", "content": ai_response})

            # 4. 如果有RAG系统，则把这段对话保存在RAG中 TODO 只获取最后两条最新的
            if self.use_rag:
                self.rag_manager.save_messages_to_rag(self.memory)

            self._log_conversation("用户", processed_user_message)
            self._log_conversation("钦灵", ai_response)
            
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
        emotion_segments = self.message_processor.analyze_emotions(ai_response)
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
                "voice_file": os.path.join(self.voice_maker.temp_voice_dir, f"part_1.{self.voice_maker.vits_tts.format}")
            }]
        
        self.voice_maker.delete_voice_files()
        await self.voice_maker.generate_voice_files(emotion_segments)
        responses = self._create_responses(emotion_segments, user_message)
    
        logger.debug("--- AI 回复分析结果 ---")
        self._log_analysis_result(emotion_segments)
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
    
    def _log_conversation(self, speaker: str, message: str):
        """记录对话日志"""
        log_message = f"{speaker}: {message}"
        logger.info_color(log_message, TermColors.WHITE)
        self.dialog_logger.log_conversation(speaker,message)

    def _log_analysis_result(self, segments):
        """记录分析结果"""
        for segment in segments:
            logger.debug(f"\n分析结果 (片段 {segment['index']}):")
            logger.debug(f"  原始标记: 【{segment['original_tag']}】")
            logger.debug(f"  中文文本: {segment['following_text']}")
            if segment['motion_text']:
                logger.debug(f"  动作文本: （{segment['motion_text']}）")
            if segment['japanese_text']:
                logger.debug(f"  日文文本: <{segment['japanese_text']}>")
            logger.debug(f"  预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
            if os.path.exists(segment['voice_file']):
                logger.debug(f"  对应语音: {os.path.basename(segment['voice_file'])}")
            else:
                if segment['japanese_text']:
                    logger.debug(f"  对应语音: (未生成或生成失败)")
    
    def _print_debug_message(self, current_context, rag_messages, messages):
        # if logger.should_print_context():
            logger.info("\n------ 开发者模式：以下信息被发送给了llm ------")
            for message in current_context:
                logger.info(f"Role: {message['role']}\nContent: {message['content']}\n")
                
            # 增加更详细的RAG信息日志
            if self.use_rag and rag_messages:
                logger.info("\n------ RAG增强信息详情 ------")
                logger.info(f"原始消息数: {len(messages)}，RAG增强后消息数: {len(current_context)}")
                logger.info(f"RAG增强消息数量: {len(rag_messages)}，位置: 系统提示后、用户消息前")
                
                # 计算并输出RAG消息的总长度（字符数）
                total_rag_chars = sum(len(msg.get('content', '')) for msg in rag_messages)
                logger.info(f"RAG增强内容总长度: {total_rag_chars} 字符")
                
                # 分析RAG消息类型统计
                role_counts = {}
                for msg in rag_messages:
                    role = msg.get('role', 'unknown')
                    role_counts[role] = role_counts.get(role, 0) + 1
                
                role_stats = ", ".join([f"{role}: {count}" for role, count in role_counts.items()])
                logger.info(f"RAG消息角色分布: {role_stats}")
                
            logger.info("------ 结束 ------")
