import os
import re
from typing import List, Dict
from datetime import datetime, timedelta

from core.pic_analyzer import DesktopAnalyzer
from core.logger import logger
from core.emotion.classifier import EmotionClassifier
from utils.function import Function


class MessageProcessor:
    def __init__(self, vits_tts) -> None:
        # 记录消息发送间隔和次数提示
        self.last_time = datetime.now()
        self.sys_time_counter = 0

        # 用于分析图像信息
        self.desktop_analyzer = DesktopAnalyzer()
        self.time_sense_enabled = os.environ.get("USE_TIME_SENSE",True)

        # 用于存储语音目录位置，其实在voice_maker已经有了
        self.voice_maker = vits_tts

    def analyze_emotions(self, text: str) -> List[Dict]:
        """分析文本中每个【】标记的情绪，并提取日语和中文部分"""
        emotion_segments = re.findall(r'(【(.*?)】)([^【】]*)', text)
        
        if not emotion_segments:
            logger.warning(f"未在文本中找到【】格式的情绪标签，将尝试添加默认标签")
            return []

        results = []
        for i, (full_tag, emotion_tag, following_text) in enumerate(emotion_segments, 1):
            following_text = following_text.replace('(', '（').replace(')', '）')

            japanese_match = re.search(r'<(.*?)>', following_text)
            japanese_text = japanese_match.group(1).strip() if japanese_match else ""

            motion_match = re.search(r'（(.*?)）', following_text)
            motion_text = motion_match.group(1).strip() if motion_match else ""

            cleaned_text = re.sub(r'<.*?>|（.*?）', '', following_text).strip()

            if japanese_text:
                japanese_text = re.sub(r'（.*?）', '', japanese_text).strip()

            if not cleaned_text and not japanese_text and not motion_text:
                continue

            try:
                if japanese_text and cleaned_text:
                    lang_jp = Function.detect_language(japanese_text)
                    lang_clean = Function.detect_language(cleaned_text)

                    if (lang_jp in ['Chinese', 'Chinese_ABS'] and lang_clean in ['Japanese', 'Chinese']) and \
                        lang_clean != 'Chinese_ABS':
                            cleaned_text, japanese_text = japanese_text, cleaned_text

            except Exception as e:
                logger.warning(f"语言检测错误: {e}")

            try:
                predicted = EmotionClassifier.get_instance().predict(emotion_tag)
                prediction_result = {
                    "label": predicted["label"],
                    "confidence": predicted["confidence"]
                }
            except Exception as e:
                logger.error(f"情绪预测错误 '{emotion_tag}': {e}")
                prediction_result = {
                    "label": "normal",
                    "confidence": 0.5
                }
            
            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

            results.append({
                "index": i,
                "original_tag": emotion_tag,
                "following_text": cleaned_text,
                "motion_text": motion_text,
                "japanese_text": japanese_text,
                "predicted": prediction_result["label"],
                "confidence": prediction_result["confidence"],
                "voice_file": os.path.join(self.voice_maker.temp_voice_dir, f"{timestamp}_part_{i}.{self.voice_maker.vits_tts.format}")
            })

        return results
    
    def append_user_message(self, user_message: str) -> str:
        """处理用户消息，添加系统信息，如时间与是否需要分析桌面"""
        current_time = datetime.now()
        processed_message = user_message

        sys_time_part = ""
        sys_desktop_part = ""
        
        if self.time_sense_enabled and ((self.last_time and 
            (current_time - self.last_time > timedelta(hours=1))) or \
            self.sys_time_counter < 1):
            
            formatted_time = current_time.strftime("%Y/%m/%d %H:%M")
            sys_time_part = f"{formatted_time} "
        
        if "看桌面" in user_message or "看看我的桌面" in user_message:
            analyze_prompt = "\"" + user_message + "\"" + "以上是用户发的消息，请切合用户实际获取信息的需要，获取桌面画面中的重点内容，用200字描述主体部分即可。"
            analyze_info = self.desktop_analyzer.analyze_desktop(analyze_prompt)
            sys_desktop_part = f"桌面信息: {analyze_info}"
        
        if sys_time_part or sys_desktop_part:
            processed_message += "\n{系统: " + (sys_time_part if sys_time_part else "") + (sys_desktop_part if sys_desktop_part else "") + "}"

        self.last_time = current_time
        self.sys_time_counter += 1

        if self.sys_time_counter >= 2:
                self.sys_time_counter = 0
        
        return processed_message
