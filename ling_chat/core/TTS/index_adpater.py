import aiohttp

from typing import Optional, AsyncGenerator
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter
from ling_chat.core.logger import logger


class IndexTTSAdapter(TTSBaseAdapter):
    def __init__(self, speaker_id: int=0, model_name: str="", 
                 audio_format: str="wav", lang: str="JP"):

        self.base_url = "https://frp-six.com:38946/voice/indextts/presets"      # TODO 记得改掉
        self.params: dict[str, int | float | str] = {
            "id": str(speaker_id),
            "emo_control_method": 1,
            "emo_id": "0",
            "vec1": 0.0,
            "vec2": 0.0,
            "vec3": 0.0,
            "vec4": 0.0,
            "vec5": 0.0,
            "vec6": 0.0,
            "vec7": 0.0,
            "vec8": 0.9,
            "emo_weight": 0.6,
            "stream": "True",
            "max_text_tokens_per_segment": 120,
            "quick_token": 80,
            "lang": "zh",
            "_verify": False  # SSL验证控制
        }

    async def generate_voice(self, text: str) -> bytes:
        # 非流式生成完整音频 TODO 建议直接切换到 indextts 的接口
        params = self.get_params()
        params["text"] = text
        params["stream"] = "False"  # 非流式
        
        async with aiohttp.ClientSession() as session:
            async with session.get(self.base_url, params=params, ssl=False) as response:
                response.raise_for_status()
                audio_data = await response.read()
                # 跳过WAV头(44字节)返回纯音频数据
                return audio_data[44:] if len(audio_data) > 44 else audio_data
            
    async def generate_voice_stream(self, text: str) -> Optional[AsyncGenerator[bytes, None]]:
        """流式生成音频"""
        params = self.get_params()
        params["text"] = text
        params["stream"] = "True"  # 确保启用流式
        
        header_buf:bytearray = bytearray()
        header_needed = 44  # WAV头长度
        header_consumed = False
        
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(self.base_url, params=params, ssl=False) as response:
                    response.raise_for_status()
                    
                    async for chunk in response.content.iter_chunked(8192):
                        if not chunk:
                            continue
                            
                        if not header_consumed:
                            # 累积头部数据
                            header_buf.extend(chunk)
                            if len(header_buf) >= header_needed:
                                # 头部已完整，返回剩余音频数据
                                audio_data = bytes(header_buf[header_needed:])
                                if audio_data:
                                    yield audio_data
                                header_consumed = True
                                header_buf = None
                        else:
                            # 直接返回音频数据
                            yield chunk
                            
        except Exception as e:
            logger.error(f"IndexTTS流式生成失败: {e}")
            raise

    def get_params(self):
        return self.params.copy()