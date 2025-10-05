import aiohttp
import os
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter
from ling_chat.core.logger import logger


class SBV2Adapter(TTSBaseAdapter):
    def __init__(self, speaker_id: int=0, model_name: str="", 
                 audio_format: str="wav", lang: str="JP"):
        # 将 lang 参数转换为 "JP"以适配sbv2的需求
        if lang == "ja":
            lang = "JP"
        
        api_url = os.environ.get("STYLE_BERT_VITS2_URL", "http://127.0.0.1:5000")
        # 处理URL末尾斜杠，避免重复
        self.api_url = api_url.rstrip('/')
        self.audio_format = audio_format
        self.params: dict[str, str|int|float] = {
            "encoding": "utf-8",  # 文本编码
            "model_name": model_name,
            "model_id": 0,  # 模型ID (0表示默认)
            "speaker_id": speaker_id,  # 说话者ID (0表示默认)
            "sdp_ratio": 0.2,  # SDP/DP混合比
            "noise": 0.6,  # 采样噪声比例
            "noisew": 0.8,  # SDP噪声
            "length": 1.0,  # 语速
            "language": lang,  # 语言 (JP/EN/ZH)
            "split_interval": 0.5,  # 分割间隔(秒)
            "style": "Neutral",  # 语音风格
            "style_weight": 1.0,  # 风格强度
            "text": ""
        }

    async def generate_voice(self, text: str) -> bytes:
        params = self.params
        params["text"] = text
        logger.debug(f"发送到SBV2的json: {params}")

        # 设置正确的Accept头
        content_types = {
            "wav": "audio/wav",
            "flac": "audio/flac", 
            "mp3": "audio/mpeg",
            "aac": "audio/aac",
            "ogg": "audio/ogg"
        }
        accept_header = content_types.get(self.audio_format, "audio/wav")

        async with aiohttp.ClientSession() as session:
            async with session.post(
                    self.api_url + "/voice",
                    params=params,
                    headers={"Accept": accept_header}
            ) as response:
                response.raise_for_status()
                return await response.read()

    def get_params(self):
        return self.params.copy()
