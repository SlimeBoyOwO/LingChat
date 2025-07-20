import os
import asyncio
import glob

from typing import List, Dict
from core.VitsTTS.vits_tts import VitsTTS
from core.logger import logger

class VoiceMaker:
    def __init__(self) -> None:
        self.vits_tts = VitsTTS()
        self.temp_voice_dir = os.environ.get("TEMP_VOICE_DIR", "frontend/public/audio")
        self.model_name = None
        self.speaker_id = 4
    
    def set_speark_id(self, id: int) -> None:
        self.speaker_id = id

    def set_model_name(self, model_name: str) -> None:
        """目前来讲，model_name会强制speaker_id变为0用于默认"""
        self.speaker_id = 0
        self.model_name = model_name

    def _init_tts_engine(self):
        """初始化TTS引擎"""
        self._prepare_directories()
    
    def _prepare_directories(self):
        """准备必要的目录"""
        os.makedirs(self.temp_voice_dir, exist_ok=True)
    
    async def generate_voice_files(self, segments: List[Dict]):
        """生成语音文件。只有在有日语文本时才生成语音"""
        tasks = []
        for seg in segments:
            if seg["japanese_text"]:
                output_file = self.vits_tts.generate_voice(seg["japanese_text"], seg["voice_file"], speaker_id=self.speaker_id, model_name=self.model_name)
                if output_file is not None:
                    tasks.append(output_file)
                else:
                    seg["voice_file"] = "none"
                
            elif seg["following_text"] and not seg.get("japanese_text"):
                logger.warning(f"片段 {seg['index']} 没有日语文本，跳过语音生成")
                
        if tasks:
            await asyncio.gather(*tasks)
    
    def delete_voice_files(self):
        self.vits_tts.cleanup()