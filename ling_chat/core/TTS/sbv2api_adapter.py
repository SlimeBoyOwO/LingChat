import aiohttp
import os
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter
from ling_chat.core.logger import logger


class SBV2APIAdapter(TTSBaseAdapter):
    def __init__(self, model_name: str="",
                 length_scale: float=1, sdp_ratio: float=0, 
                 speaker_id: int=0, style_id: int=0,
                 audio_format: str="wav"):
        
        api_url = os.environ.get("SBV2API_API_URL", "http://localhost:3000")
        # 处理URL末尾斜杠，避免重复
        self.api_url = api_url.rstrip('/')
        self.params: dict[str, str|int|float] = {
            "ident": model_name,
            "length_scale": length_scale,
            "sdp_ratio": sdp_ratio,
            "speaker_id": speaker_id,
            "style_id": style_id,
            "text": ""
        }
        self.format = audio_format

    async def generate_voice(self, text: str) -> bytes:
        params = self.params
        params["text"] = text
        logger.debug("发送到SBV2API的json:" + str(params))

        async with aiohttp.ClientSession() as session:
            async with session.post(
                    self.api_url + "/synthesize",
                    json=params
            ) as response:
                if response.status != 200:
                    try:
                        error_detail = await response.json()
                    except:
                        error_detail = await response.text()
                    raise Exception(f"HTTP {response.status}: {error_detail}")
                return await response.read()

    def get_params(self):
        return self.params.copy()