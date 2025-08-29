import asyncio

from typing import List, Dict, Awaitable
from ling_chat.core.TTS.tts_provider import TTS
from ling_chat.core.logger import logger

class VoiceMaker:
    def __init__(self) -> None:
        self.tts_provider = TTS()
        self.model_name = ""
        self.speaker_id = 4
        self.tts_type = ""
        self.lang = "ja"  # 默认语言为日语
    
    def set_tts_settings(self, tts_settings: dict[str,str], name: str) -> None:
        """获取可用的TTS配置并且进行基础配置"""
        try:
            if self.tts_type == "sva":
                self.tts_provider.init_sva_adapter(speaker_id=int(tts_settings["sva_speaker_id"]))
            elif self.tts_type == "sbv2":
                self.tts_provider.init_sbv2_adapter(speaker_id=int(tts_settings["sbv2_speaker_id"]), 
                                                     model_name=tts_settings["sbv2_name"])
            elif self.tts_type == "bv2":
                self.tts_provider.init_bv2_adapter(speaker_id=int(tts_settings["bv2_speaker_id"]), )
            elif self.tts_type == "sbv2api":
                self.tts_provider.init_sbv2api_adapter(model_name=tts_settings["sbv2api_name"],
                                                       speaker_id=int(tts_settings["sbv2api_speaker_id"]))
            elif self.tts_type == "gsv":
                self.tts_provider.init_gsv_adapter(ref_audio_path=tts_settings["gsv_voice_filename"], 
                                                     prompt_text=tts_settings["gsv_voice_text"], )
        except KeyError as e:
            logger.error(f"当前角色卡{name}的TTS设置出错，问题是：{e}")

    def set_tts_type(self, tts_type: str) -> None:
        """设置默认的TTS类型"""
        if tts_type in ("bv2", "gsv", "sbv2", "sva", "sbv2api"):
            self.tts_type = tts_type
        else:
            raise ValueError(f"未知的TTS类型: {tts_type}")
    
    def set_lang(self, lang: str) -> None:
        """设置语言"""
        if lang not in ["ja", "zh"]:
            raise ValueError(f"不支持的语言: {lang}")
        self.lang = lang
    
    async def generate_voice_files(self, segments: List[Dict[str, str]]):
        """生成语音文件"""
        tasks: List[Awaitable[str | None]] = []
        logger.debug(f"生成语音文件: {segments}")
        for seg in segments:
            if self.lang == "ja":
                if seg["japanese_text"]:
                    task = self.tts_provider.generate_voice(seg["japanese_text"], 
                                                            seg["voice_file"], 
                                                            tts_type=self.tts_type, 
                                                            lang="ja")
                    tasks.append(task)
                elif seg["following_text"] and not seg.get("japanese_text"):
                    logger.warning(f"片段 {seg['index']} 没有日语文本，跳过语音生成")
            elif self.lang == "zh":
                if seg["following_text"]:
                    task = self.tts_provider.generate_voice(seg["following_text"], 
                                                            seg["voice_file"], 
                                                            tts_type=self.tts_type, 
                                                            lang="zh")
                    tasks.append(task)
                else:
                    logger.warning(f"片段 {seg['index']} 没有中文文本，跳过语音生成\n"
                                   f"Tips：要真出现这情况，你应该检查LLM是否正常输出。")
        if tasks:
            await asyncio.gather(*tasks)
