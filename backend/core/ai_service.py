import os
import glob
import asyncio
import re
from typing import List, Dict, Optional

from .deepseek import DeepSeek
from .predictor import EmotionClassifier  # 导入情绪分类器
from .VitsTTS import VitsTTS              # 导入语音生成
from .logger import Logger
from .langDetect import LangDetect

# 常量定义
TEMP_VOICE_DIR = "../public/audio"
WS_HOST = "localhost"
WS_PORT = 8765

# ANSI 颜色代码
COLOR = {
    "user": "\033[92m",
    "ai": "\033[96m",
    "emotion": "\033[93m",
    "reset": "\033[0m"
}

class AIService:
    def __init__(self, logger=None):
        """初始化所有服务组件"""
        self.logger = logger or Logger()
        self.deepseek = DeepSeek(logger=self.logger)
        self.emotion_classifier = EmotionClassifier(logger=self.logger)
        self.lang_detector = LangDetect()
        self.tts_engine = self._init_tts_engine()
        self._prepare_directories()

        self.temp_voice_dir = os.environ.get("TEMP_VOICE_DIR", "frontend/public/audio")
        os.makedirs(self.temp_voice_dir, exist_ok=True)
        
    def _init_tts_engine(self) -> VitsTTS:
        """初始化TTS引擎"""
        return VitsTTS(
            api_url="http://127.0.0.1:23456/voice/vits",
            speaker_id=4,
            lang="ja",
            logger=self.logger
        )
    
    def _prepare_directories(self):
        """准备必要的目录"""
        os.makedirs(TEMP_VOICE_DIR, exist_ok=True)
    
    async def process_message(self, user_message: str) -> Optional[List[Dict]]:
        """处理用户消息的完整流程"""
        try:
            # 1. 获取AI回复
            ai_response = self.deepseek.process_message(user_message)
            self._log_conversation("用户", user_message)
            self._log_conversation("钦灵", ai_response)
            
            # 2. 分析情绪和生成语音
            return await self._process_ai_response(ai_response, user_message)
        except Exception as e:
            self.logger.error(f"处理消息时出错: {e}")
            return None
    
    def load_memory(self, memory):
        self.deepseek.load_memory(memory)
        self.logger.info("新的记忆已经被加载")
    
    async def _process_ai_response(self, ai_response: str, user_message: str) -> List[Dict]:
        """处理AI回复的完整流程"""
        self._clean_temp_voice_files()
        
        # 分析情绪片段
        emotion_segments = self._analyze_emotions(ai_response)
        if not emotion_segments:
            self.logger.warning("未检测到有效情绪片段")
            raise ValueError("未检测到有效情绪片段")
        
        # 生成语音和构造响应
        await self._generate_voice_files(emotion_segments)
        responses = self._create_responses(emotion_segments, user_message)
    
        self.logger.debug("--- AI 回复分析结果 ---")
        self._log_analysis_result(emotion_segments)
        self.logger.debug("--- 分析结束 ---")

        return responses
    
    def _analyze_emotions(self, text: str) -> List[Dict]:
        """分析文本中每个【】标记的情绪，并提取日语和中文部分"""
        # 改进后的正则表达式，更灵活地匹配各种情况
        emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)

        results = []
        for i, (full_tag, emotion_tag, following_text) in enumerate(emotion_segments, 1):
            # 统一处理括号（兼容中英文括号）
            following_text = following_text.replace('(', '（').replace(')', '）')

            # 提取日语部分（<...>），改进匹配模式
            japanese_match = re.search(r'<(.*?)>', following_text)
            japanese_text = japanese_match.group(1).strip() if japanese_match else ""

            # 提取动作部分（（...）），改进匹配模式
            motion_match = re.search(r'（(.*?)）', following_text)
            motion_text = motion_match.group(1).strip() if motion_match else ""

            # 清理后的文本（移除日语部分和动作部分）
            cleaned_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()

            # 清理日语文本中的动作部分
            if japanese_text:
                japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()

            # 跳过完全空的文本
            # 修改：如果cleaned_text和japanese_text都为空，但motion_text不为空，也应保留
            if not cleaned_text and not japanese_text and not motion_text:
                continue # 只有三者都为空时才跳过

            # 改进语言检测和处理
            try:
                if japanese_text and cleaned_text:
                    # 如果两者都有内容，才进行语言检测和交换
                    lang_jp = self.lang_detector.detect_language(japanese_text)
                    lang_clean = self.lang_detector.detect_language(cleaned_text)

                    if (lang_jp in ['Chinese', 'Chinese_ABS'] and lang_clean in ['Japanese', 'Chinese']) and \
                        lang_clean != 'Chinese_ABS':
                            cleaned_text, japanese_text = japanese_text, cleaned_text

            except Exception as e:
                # 语言检测失败时保持原样
                self.logger.warning(f"语言检测错误: {e}")

            # 对情绪标签单独预测，增加错误处理
            try:
                predicted = self.emotion_classifier.predict(emotion_tag)
                prediction_result = {
                    "label": predicted["label"],
                    "confidence": predicted["confidence"]
                }
            except Exception as e:
                self.logger.error(f"情绪预测错误 '{emotion_tag}': {e}")
                prediction_result = {
                    "label": "unknown",
                    "confidence": 0.0
                }

            results.append({
                "index": i,
                "original_tag": emotion_tag,
                "following_text": cleaned_text,
                "motion_text": motion_text,
                "japanese_text": japanese_text,
                "predicted": prediction_result["label"],
                "confidence": prediction_result["confidence"],
                # 使用 os.path.basename 确保只包含文件名
                "voice_file": os.path.join(self.temp_voice_dir, f"part_{i}.{self.tts_engine.format}")
            })

        return results
    
    async def _generate_voice_files(self, segments: List[Dict]):
        """生成语音文件"""
        tasks = [
            self.tts_engine.generate_voice(seg["japanese_text"], seg["voice_file"], True)
            for seg in segments if seg["japanese_text"]
        ]
        await asyncio.gather(*tasks)
    
    def _create_responses(self, segments: List[Dict], user_message: str) -> List[Dict]:
        """构造响应消息"""
        total_parts = len(segments)
        return [{
            "type": "reply",
            "emotion": seg['predicted'],
            "originalTag": seg['original_tag'],
            "message": seg['following_text'],
            "motionText": seg['motion_text'],
            "audioFile": os.path.basename(seg['voice_file']) if os.path.exists(seg['voice_file']) else None,
            "originalMessage": user_message,
            "isMultiPart": total_parts > 1,
            "partIndex": idx,
            "totalParts": total_parts
        } for idx, seg in enumerate(segments)]
    
    def _clean_temp_voice_files(self):
        """清理临时语音文件"""
        for file in glob.glob(os.path.join(TEMP_VOICE_DIR, "*.wav")):
            try:
                os.remove(file)
            except OSError as e:
                self.logger.warning(f"删除文件 {file} 失败: {e}")
    
    def _log_conversation(self, speaker: str, message: str):
        """记录对话日志"""
        log_message = f"{speaker}: {message}"
        self.logger.info_white_text(log_message)
        self.logger.log_conversation(speaker, message)

    def _log_analysis_result(self, segments):
        """记录分析结果"""
        for segment in segments:
            self.logger.debug(f"\n分析结果 (片段 {segment['index']}):")
            self.logger.debug(f"  原始标记: 【{segment['original_tag']}】")
            self.logger.debug(f"  中文文本: {segment['following_text']}")
            if segment['motion_text']:
                self.logger.debug(f"  动作文本: （{segment['motion_text']}）")
            if segment['japanese_text']:
                self.logger.debug(f"  日文文本: <{segment['japanese_text']}>")
            self.logger.debug(f"  预测情绪: {segment['predicted']} (置信度: {segment['confidence']:.2%})")
            if os.path.exists(segment['voice_file']):
                self.logger.debug(f"  对应语音: {os.path.basename(segment['voice_file'])}")
            else:
                if segment['japanese_text']:
                    self.logger.debug(f"  对应语音: (未生成或生成失败)")
    
    def get_memory(self):
        return self.deepseek.get_messsages()