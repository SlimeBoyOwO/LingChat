import aiohttp
from .base_adapter import TTSBaseAdapter

class GPTSoVITSAdapter(TTSBaseAdapter):
    def __init__(self, api_url, ref_audio_path="", prompt_text="", prompt_lang="zh"):
        self.api_url = api_url
        self.ref_audio_path = ref_audio_path
        self.prompt_text = prompt_text
        self.prompt_lang = prompt_lang

    async def generate_voice(self, text: str, params: dict) -> bytes:
        payload = {
            "text": text,
            "text_lang": params.get("text_lang", "auto"),
            "ref_audio_path": self.ref_audio_path,
            "prompt_text": self.prompt_text,
            "prompt_lang": self.prompt_lang,
            "media_type": params.get("media_type", "wav"),
            "speed_factor": params.get("speed_factor", 1.0),
            "text_split_method": params.get("text_split_method", "cut0"),
            "top_k": 15,
            "top_p": 100,
            "temperature": 1
        }
        async with aiohttp.ClientSession() as session:
            async with session.post(self.api_url, json=payload) as resp:
                if resp.status != 200:
                    raise RuntimeError(f"TTS请求失败: {await resp.text()}")
                return await resp.read()

    def get_default_params(self) -> dict:
        return {
            "text_lang": "zh",
            "media_type": "wav",
            "speed_factor": 1.0,
            "text_split_method": "cut0"
        }
