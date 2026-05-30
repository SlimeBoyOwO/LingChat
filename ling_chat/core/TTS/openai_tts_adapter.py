import os

from openai import AsyncOpenAI

from ling_chat.core.logger import logger
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter


class OpenAITTSAdapter(TTSBaseAdapter):
    """OpenAI TTS API 适配器

    兼容 SiliconFlow、Azure OpenAI TTS 等 OpenAI TTS API 格式的服务商。
    参考 AstrBot 的 ProviderOpenAITTSAPI 实现。
    """

    def __init__(
        self,
        model: str = "",
        voice: str = "",
        audio_format: str = "wav",
    ):
        api_key = os.environ.get("OPENAI_TTS_API_KEY", "")
        base_url = os.environ.get("OPENAI_TTS_BASE_URL", "https://api.openai.com/v1")

        if not api_key:
            raise ValueError("未设置 OPENAI_TTS_API_KEY 环境变量")

        self.client = AsyncOpenAI(api_key=api_key, base_url=base_url)
        self.model = model
        self.voice = voice
        self.format = audio_format

        self._params = {
            "model": model,
            "voice": voice,
            "response_format": audio_format,
        }

    async def generate_voice(self, text: str) -> bytes:
        if not self.model:
            raise ValueError("OpenAI TTS 模型未设置")
        if not self.voice:
            raise ValueError("OpenAI TTS 音色(voice)未设置")

        logger.debug(f"OpenAI TTS 请求: model={self.model}, voice={self.voice}, text={text[:50]}...")

        response = await self.client.audio.speech.create(
            model=self.model,
            voice=self.voice,
            input=text,
            response_format=self.format,  # type: ignore[arg-type]
        )

        audio_bytes = response.content
        logger.debug(f"OpenAI TTS 响应成功: {len(audio_bytes)} bytes")
        return audio_bytes

    def get_params(self) -> dict[str, str | int | float | bool]:
        return self._params.copy()
