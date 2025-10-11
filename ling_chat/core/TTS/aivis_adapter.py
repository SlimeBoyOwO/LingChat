import aiohttp
import os
from ling_chat.core.TTS.base_adapter import TTSBaseAdapter
from ling_chat.core.logger import logger


class AIVISAdapter(TTSBaseAdapter):
    def __init__(self, model_uuid: str, speaker_uuid: str|None = None,
                 style_id: int|None = None, style_name: str|None = None, 
                 audio_format: str = "mp3", lang: str = "ja"
                ):
        """
        初始化AIVIS适配器
        
        :param api_url: AIVIS API地址
        :param model_uuid: 语音合成模型的UUID (必须)
        :param speaker_uuid: 语音合成模型的说话人UUID (可选)
        :param style_id: 风格ID (0-31, 与style_name互斥)
        :param style_name: 风格名称 (与style_id互斥)
        :param audio_format: 音频格式 (wav/flac/mp3/aac/opus)
        :param lan: 语言 (BCP47格式, 默认"ja"，目前仅支持ja)
        """
        api_url = os.environ.get("AIVIS_API_URL", "https://api.aivis-project.com/v1")
        # 处理URL末尾斜杠，避免重复
        self.api_url = api_url.rstrip('/')
        self.api_key = os.environ.get("AIVIS_API_KRY", "")
        
        self.params: dict[str, str|int|float|bool|None] = {
            "model_uuid": model_uuid,
            "speaker_uuid": speaker_uuid,
            "style_id": style_id,
            "style_name": style_name,
            "language": lang,
            "use_ssml": True,
            "speaking_rate": 1.0,
            "emotional_intensity": 1.0,
            "tempo_dynamics": 1.0,
            "pitch": 0.0,
            "volume": 1.0,
            "leading_silence_seconds": 0.1,
            "trailing_silence_seconds": 0.1,
            "line_break_silence_seconds": 0.4,
            "output_format": audio_format,
            "output_sampling_rate": 44100,
            "output_audio_channels": "mono",
            "text": ""
        }

        # 移除值为None的参数
        self.params = {k: v for k, v in self.params.items() if v is not None}
        
        # 验证style_id和style_name是否同时指定
        if style_id is not None and style_name is not None:
            raise ValueError("style_id和style_name不能同时指定，只能选择其一")

    async def generate_voice(self, text: str) -> bytes:
        """
        生成语音
        
        :param text: 要转换为语音的文本
        :return: 音频数据的字节流
        """
        params = self.params.copy()
        params["text"] = text
        logger.debug("发送到AIVIS的参数: %s" +str(params))

        # 设置正确的Accept头
        output_format = params.get("output_format", "mp3")
        content_types = {
            "wav": "audio/wav",
            "flac": "audio/flac", 
            "mp3": "audio/mpeg",
            "aac": "audio/aac",
            "opus": "audio/ogg; codecs=opus"
        }
        accept_header = content_types.get(str(output_format), "audio/mpeg")

        # 构建请求头
        headers = {
            "Accept": accept_header, 
            "Content-Type": "application/json"
        }
        headers["Authorization"] = f"Bearer {self.api_key}"

        async with aiohttp.ClientSession() as session:
            async with session.post(
                    self.api_url + "/tts/synthesize",
                    json=params,
                    headers=headers
            ) as response:
                if response.status >= 400:
                    error_text = await response.text()
                    logger.error(f"AIVIS API错误({response.status}): {error_text}")
                
                response.raise_for_status()
                return await response.read()

    def get_params(self):
        """
        获取当前适配器的参数
        
        :return: 参数字典
        """
        return {k: v for k, v in self.params.items() 
                if isinstance(v, (str, int, float, bool))}
