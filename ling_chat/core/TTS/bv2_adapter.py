import aiohttp
import os
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter
from ling_chat.core.logger import logger

class BV2Adapter(TTSBaseAdapter):
    def __init__(self, speaker_id: int=0, 
                 audio_format: str="wav", lang: str="zh"):
        api_url = os.environ.get("SIMPLE_VITS_API_URL", "http://127.0.0.1:6006")
        # 处理URL末尾斜杠，避免重复
        self.api_url = api_url.rstrip('/')
        self.params: dict[str, str|int|float] = {
            "id": speaker_id,
            "format": audio_format,   # 可用wav,ogg,silk,mp3,flac
            "lang": lang,   # 语言 (Auto/zh/ja)
            "length": 1.0, # 语速
            "noise": 0.33, # 采样噪声比例
            "noisew": 0.4, # SDP噪声
            "segment_size": 50, #分段阈值
            "sdp_radio": 0.2, # SDP/DP混合比
            "text": ""
        }

    async def generate_voice(self, text: str) -> bytes:
        params = self.params
        params["text"] = text
        logger.debug(f"发送到SVA-BV2的json: {params}")

        async with aiohttp.ClientSession() as session:
            async with session.post(
                self.api_url + "/voice/bert-vits2", 
                json=params
            ) as response:
                if response.status != 200:
                    raise RuntimeError(f"TTS请求失败: {await response.text()}")
                return await response.read()
    
    def get_params(self):
        return self.params
