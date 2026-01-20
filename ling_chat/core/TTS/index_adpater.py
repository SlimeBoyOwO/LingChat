from typing import AsyncGenerator, Optional

import aiohttp

from ling_chat.core.logger import logger
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter


class IndexTTSAdapter(TTSBaseAdapter):
    def __init__(self, speaker_id: int=0, model_name: str="",
                 audio_format: str="wav", lang: str="zh"):

        self.base_url = "http://127.0.0.1:23467/voice/indextts/presets"
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
            "vec8": 0.0,
            "emo_weight": 0.6,
            "stream": "True",
            "max_text_tokens_per_segment": 120,
            "quick_token": 0,
            "lang": lang,
            "audio_format": audio_format,
            "_verify": 0  # SSL验证控制
        }

    async def generate_voice(self, text: str, emo: str = "") -> bytes:
        # 非流式生成完整音频 TODO 建议直接切换到 indextts 的接口
        params = self.get_params()
        params["text"] = text
        params["emo_id"] = emo
        params["stream"] = "False"  # 非流式
        logger.debug("开始调用IndexTTS生成语音...")

        async with aiohttp.ClientSession() as session:
            async with session.get(self.base_url, params=params, ssl=False) as response:
                response.raise_for_status()
                audio_data = await response.read()
                return audio_data

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

if __name__ == "__main__":
    # test generate_voice
    import asyncio
    async def test():
        adapter = IndexTTSAdapter(speaker_id=0)
        audio_data = await adapter.generate_voice("欢迎使用Stream api for Index TTS 语音合成服务！")
        with open("test_index_tts.wav", "wb") as f:
            # 写入WAV头（简单示例，实际应用中应根据音频格式生成正确的头部）
            f.write(b'RIFF' + (36 + len(audio_data)).to_bytes(4, 'little') +
                    b'WAVEfmt ' + (16).to_bytes(4, 'little') +
                    (1).to_bytes(2, 'little') + (1).to_bytes(2, 'little') +
                    (22050).to_bytes(4, 'little') + (22050 * 2).to_bytes(4, 'little') +
                    (2).to_bytes(2, 'little') + (16).to_bytes(2, 'little') +
                    b'data' + len(audio_data).to_bytes(4, 'little'))
            f.write(audio_data)
        logger.info("IndexTTS语音生成测试完成，文件保存为 test_index_tts.wav")

    asyncio.run(test())
