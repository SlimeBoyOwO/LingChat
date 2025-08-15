import aiohttp
from .base_adapter import BaseVitsAdapter

class BV2VitsAdapter(BaseVitsAdapter):
    def __init__(self, api_url, speaker_id=0, audio_format="wav", lang="zh"):
        self.api_url = api_url
        self.speaker_id = speaker_id
        self.audio_format = audio_format
        self.lang = lang

    async def generate_voice(self, text: str, params: dict) -> bytes:
        payload = {
            "id": self.speaker_id,
            "format": self.audio_format,
            "lang": self.lang,
            "length": params.get("length", 1.0), #语速
            "noise": params.get("noise", 0.33), # 采样噪声比例
            "noisew": params.get("noisew", 0.4), # SDP噪声
            "segment_size": params.get("segment_size", 50), #分段阈值
            "sdp_radio": params.get("sdp_radio", 0.2), # SDP/DP混合比
            "text": text
        }
        async with aiohttp.ClientSession() as session:
            async with session.post(self.api_url, json=payload) as resp:
                if resp.status != 200:
                    raise RuntimeError(f"TTS请求失败: {await resp.text()}")
                return await resp.read()
    
    def get_default_params(self) -> dict:
        return {
            "length": 1.0,
            "noise": 0.33,
            "noisew": 0.4,
            "segment_size": 50,
            "sdp_radio": 0.2,
        }

