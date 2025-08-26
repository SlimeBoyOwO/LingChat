import os
import asyncio
import glob

from typing import List, Dict
from ling_chat.core.TTS.tts_provider import TTS
from ling_chat.core.logger import logger

class VoiceMaker:
    def __init__(self) -> None:
        self.tts_provider = TTS()
        self.model_name = None
        self.speaker_id = 4
        self.tts_type = ""
        self.lang = "ja"  # 默认语言为日语
    
    def set_speark_id(self, id: int) -> None:
        self.speaker_id = id

    def set_model_name(self, model_name: str) -> None:
        """目前来讲，model_name会强制speaker_id变为0用于默认"""
        self.speaker_id = 0
        self.model_name = model_name

    def set_tts_type(self, tts_type: str) -> None:
        """设置TTS类型"""
        if tts_type in ("bv2", "gsv", "sbv", "sva"):
            self.tts_type = tts_type
        else:
            raise ValueError(f"未知的TTS类型: {tts_type}")
    
    def set_lang(self, lang: str) -> None:
        """设置语言"""
        if lang not in ["ja", "zh"]:
            raise ValueError(f"不支持的语言: {lang}")
        self.lang = lang
    
    async def generate_voice_files(self, segments: List[Dict]):
        """生成语音文件"""
        tasks = []
        for seg in segments:
            # logger.debug(seg)
            if self.lang == "ja":
                if seg["japanese_text"]:
                    output_file = self.tts_provider.generate_voice(seg["japanese_text"], seg["voice_file"], speaker_id=self.speaker_id, model_name=self.model_name, tts_type=self.tts_type, lang="ja")
                    if output_file is not None:
                        tasks.append(output_file)
                    else:
                        seg["voice_file"] = "none"
                elif seg["following_text"] and not seg.get("japanese_text"):
                    logger.warning(f"片段 {seg['index']} 没有日语文本，跳过语音生成")
            elif self.lang == "zh":
                output_file = self.tts_provider.generate_voice(seg["following_text"], seg["voice_file"], speaker_id=self.speaker_id, model_name=self.model_name, tts_type=self.tts_type, lang="zh")
                if output_file is not None:
                    tasks.append(output_file)
                else:
                    seg["voice_file"] = "none"
        if tasks:
            await asyncio.gather(*tasks)
    
    def delete_voice_files(self):
        self.tts_provider.cleanup()