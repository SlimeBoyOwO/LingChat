import aiohttp
import os
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter
from ling_chat.core.logger import logger

class GPTSoVITSAdapter(TTSBaseAdapter):
    def __init__(self, ref_audio_path: str, 
                 prompt_text: str="", prompt_lang: str="zh",
                 audio_format: str="wav", text_lang: str="auto",
                 parallel_infer: bool=True
                ):
        api_url = os.environ.get("GPT_SOVITS_API_URL", "http://127.0.0.1:9880")
        # 处理URL末尾斜杠，避免重复
        self.api_url = api_url.rstrip('/')
        
        # 支持的语言（v2及以上）：
        # auto 多语种自动识别切分
        # en	英语
        # zh	中英混合识别
        # ja	日英混合识别
        # yue	粤英混合识别
        # ko	韩英混合识别
        # all_zh	全部按中文识别
        # all_ja	全部按日文识别
        # all_yue	全部按粤语识别
        # all_ko	全部按韩文识别
        # auto_yue	粤语多语种自动识别切分
        self.params: dict[str, str|int|float] = {
            "ref_audio_path": ref_audio_path,
            "prompt_text": prompt_text,
            "prompt_lang": prompt_lang,
            "text_lang": text_lang, 
            "media_type": audio_format, # 支持wav,raw,ogg,aac
            "speed_factor": 1.0,
            "text_split_method": "cut0",
            "top_k": 15,
            "top_p": 100.0,
            "temperature": 1.0,
            "parallel_infer": parallel_infer,
            "text": ""
        }

    async def generate_voice(self, text: str) -> bytes:
        params = self.params
        params["text"] = text
        logger.debug(f"发送到GPT-SoVITS的json: {params}")

        async with aiohttp.ClientSession() as session:
            async with session.post(
                self.api_url + "/tts",
                json=params
            ) as resp:
                if resp.status != 200:
                    raise RuntimeError(f"TTS请求失败: {await resp.text()}")
                return await resp.read()

    def get_params(self):
        return self.params