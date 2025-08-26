import aiohttp
import json
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter
from ling_chat.core.logger import logger


class SBV2APIAdapter(TTSBaseAdapter):
    def __init__(self, api_url, model_name="", length_scale=1, sdp_ratio=0, 
                 speaker_id=0, style_id=0, audio_format="wav"):
        self.api_url = api_url
        self.default_params = {
            "ident": model_name,
            "length_scale": length_scale,
            "sdp_ratio": sdp_ratio,
            "speaker_id": speaker_id,
            "style_id": style_id
        }
        self.format = audio_format

    async def generate_voice(self, text: str, params: dict) -> bytes:
        merged_params = {**self.default_params, **params}
        merged_params["text"] = text
        logger.debug("发送到SBV2API的json:"+json.dumps(merged_params, ensure_ascii=False, indent=2))

        async with aiohttp.ClientSession() as session:
            async with session.post(
                    self.api_url,
                    json=merged_params
            ) as response:
                if response.status != 200:
                    try:
                        error_detail = await response.json()
                    except:
                        error_detail = await response.text()
                    raise Exception(f"HTTP {response.status}: {error_detail}")
                return await response.read()

    def get_default_params(self) -> dict:
        return self.default_params.copy()