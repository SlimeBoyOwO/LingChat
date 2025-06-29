import aiohttp
from .base_adapter import BaseVitsAdapter


class SBVVitsAdapter(BaseVitsAdapter):
    def __init__(self, api_url, speaker_id=0, model_name=0, audio_format="wav", lang="JP"):
        self.api_url = api_url
        self.default_params = {
            "encoding": "utf-8",  # 文本编码
            "model_name": model_name,
            "model_id": 0,  # 模型ID (0表示默认)
            "speaker_id": 0,  # 说话者ID (0表示默认)
            "sdp_ratio": 0.2,  # SDP/DP混合比
            "noise": 0.6,  # 采样噪声比例
            "noisew": 0.8,  # SDP噪声
            "length": 1.0,  # 语速
            "language": lang,  # 语言 (JP/EN/ZH)
            "split_interval": 0.5,  # 分割间隔(秒)
            "style": "Neutral",  # 语音风格
            "style_weight": 1.0  # 风格强度
        }

    async def generate_voice(self, text: str, params: dict) -> bytes:
        merged_params = {**self.default_params, **params}
        merged_params["text"] = text
        merged_params["speaker_id"] = 0

        async with aiohttp.ClientSession() as session:
            async with session.post(
                    self.api_url,
                    params=merged_params,
                    headers={"Accept": f"audio/{merged_params.get('format', 'wav')}"}
            ) as response:
                response.raise_for_status()
                return await response.read()

    def get_default_params(self) -> dict:
        return self.default_params.copy()
