import aiohttp
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter


class SVAAdapter(TTSBaseAdapter):
    def __init__(self, api_url, speaker_id=4, audio_format="wav", lang="ja"):
        self.api_url = api_url
        self.default_params = {
            "id": speaker_id,
            "format": audio_format,
            "lang": lang
        }

    async def generate_voice(self, text: str, params: dict) -> bytes:
        merged_params = {**self.default_params, **params}
        merged_params["text"] = text

        async with aiohttp.ClientSession() as session:
            async with session.get(self.api_url, params=merged_params) as response:
                response.raise_for_status()
                return await response.read()

    def get_default_params(self) -> dict:
        return self.default_params.copy()
