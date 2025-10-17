import os
from pathlib import Path
from ling_chat.core.TTS.vits_adapter import VitsAdapter
from ling_chat.core.TTS.sbv2_adapter import SBV2Adapter
from ling_chat.core.TTS.gsv_adapter import GPTSoVITSAdapter
from ling_chat.core.TTS.sbv2api_adapter import SBV2APIAdapter
from ling_chat.core.TTS.bv2_adapter import BV2Adapter
from ling_chat.core.TTS.aivis_adapter import AIVISAdapter
from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import temp_path


class TTS:
    def __init__(self, 
                 default_speaker_id: int=4,
                 default_model_name: str="",
                 default_tts_type: str = "sbv2",
                 default_language: str = "ja"
                 ):
        """
        初始化TTS语音合成器

        :param default_speaker_id: 默认说话人ID
        :param default_model_name: 默认模型名称
        :param default_tts_type: 默认TTS类型
        :param default_language: 默认语言
        """
        self.default_speaker_id = default_speaker_id
        self.default_model_name = default_model_name
        self.default_tts_type = default_tts_type
        self.default_language = default_language

        self.format = os.environ.get("VOICE_FORMAT", "wav")

        self.audio_format = self.format
        self.temp_dir = Path(os.environ.get("TEMP_VOICE_DIR", temp_path / "data/voice"))
        self.temp_dir.mkdir(parents=True, exist_ok=True)
        self.enable = True  # 初始化时启用
        
        # 提前初始化适配器属性为None，之后就可用判断了（pylance如是说）
        self.sva_adapter = None
        self.sbv2_adapter = None
        self.sbv2api_adapter = None
        self.bv2_adapter = None
        self.gsv_adapter = None
        self.aivis_adapter = None

    def init_sva_adapter(self,speaker_id: int):
        """
        初始化SVA适配器

        :param speaker_id: 说话人ID
        """
        self.sva_adapter = VitsAdapter(
            speaker_id = speaker_id,
            audio_format = self.format,
            lang = "ja"
        )
    
    def init_sbv2_adapter(self, speaker_id: int, model_name: str, language: str="ja"):
        """
        初始化SBV2适配器

        :param speaker_id: 说话人ID
        :param model_name: 模型名称
        :param language: 语言选择
        """
        self.sbv2_adapter = SBV2Adapter(
            speaker_id = speaker_id,
            model_name = model_name,
            audio_format = self.format,
            lang = language
        )

    def init_sbv2api_adapter(self, model_name: str, speaker_id: int):
        """
        初始化SBV2API适配器

        :param model_name: 模型名称
        :param speaker_id: 说话人ID
        """
        self.sbv2api_adapter = SBV2APIAdapter(
            model_name = model_name,
            speaker_id= speaker_id,
            audio_format = self.format
        )
        
    def init_bv2_adapter(self, speaker_id: int, language: str="zh"):
        """
        初始化BV2适配器

        :param speaker_id: 说话人ID
        :param language: 语言选择
        """
        self.bv2_adapter = BV2Adapter(
            speaker_id = speaker_id,
            audio_format = self.format,
            lang = language
        )

    def init_aivis_adapter(self, model_uuid: str, speaker_uuid: str|None = None, language: str="ja"):
        """
        初始化AIVIS适配器

        :param model_uuid: 模型UUID
        :param speaker_uuid: 说话人UUID（可选）
        :param language: 语言选择（目前仅支持ja）
        """
        if os.environ.get("AIVIS_API_KRY", "") == "":
            logger.warning("未设置AIVIS_API_KRY环境变量，请检查是否正确设置")
            self.enable = False
            return None
        self.aivis_adapter = AIVISAdapter(
            model_uuid=model_uuid,
            speaker_uuid=speaker_uuid,
            audio_format=self.format,
            lang=language
        )

    def init_gsv_adapter(self, ref_audio_path: str, prompt_text: str, prompt_lang: str = "auto"):
        """
        初始化GSV适配器

        :param ref_audio_path: 参考音频路径
        :param prompt_text: 提示文本
        :param prompt_lang: 提示语言，默认为"auto"
        """
        self.gsv_adapter = GPTSoVITSAdapter(
            ref_audio_path = ref_audio_path,
            prompt_text = prompt_text,
            prompt_lang = prompt_lang
        )

    def _select_adapter(self, tts_type: str):
        """
        根据tts_type选择适配器(如果传入),为空则自动选择

        :param tts_type: TTS类型字符串
        :return: 对应的TTS适配器实例
        :raises ValueError: 当指定的适配器未初始化或TTS类型未知时抛出异常
        """
        if tts_type != "":
            logger.debug(f"根据参数选择TTS适配器: {tts_type}")

            if tts_type == 'sva-vits':
                if self.sva_adapter is None:
                    raise ValueError("Vits适配器未初始化")
                return self.sva_adapter
            elif tts_type == 'sbv2':
                if self.sbv2_adapter is None:
                    raise ValueError("Style-Bert-Vits2适配器未初始化")
                return self.sbv2_adapter
            elif tts_type == 'gsv':
                if self.gsv_adapter is None:
                    raise ValueError("GPT-SoVITS适配器未初始化")
                return self.gsv_adapter
            elif tts_type == 'sva-bv2':
                if self.bv2_adapter is None:
                    raise ValueError("Bert-Vits2适配器未初始化")
                return self.bv2_adapter
            elif tts_type == 'sbv2api':
                if self.sbv2api_adapter is None:
                    raise ValueError("sbv2-api适配器未初始化")
                return self.sbv2api_adapter
            elif tts_type == 'aivis':
                if self.aivis_adapter is None:
                    raise ValueError("AIVIS适配器未初始化")
                return self.aivis_adapter
            else:
                raise ValueError(f"未知的TTS类型: {tts_type}")
        elif self.sbv2_adapter is not None:
            logger.warning("未指定tts_type,默认使用sbv2")
            return self.sbv2_adapter
        else:
            raise ValueError("没有可用的API适配器")

    async def generate_voice(self, text: str, file_name: str, 
                             tts_type: str = "", lang: str ="ja") -> str | None:
        """
        生成语音文件

        :param text: 要转换为语音的文本
        :param file_name: 输出文件名
        :param tts_type: TTS类型，默认为空字符串表示自动选择
        :param lang: 语言，默认为"ja"
        :return: 成功时返回输出文件路径，失败时返回None
        """
        if not self.enable:
            logger.warning("TTS服务未启用，跳过语音生成")
            return None

        if not text or not text.strip():
            logger.debug("提供的文本为空，跳过语音生成")
            return None

        try:
            # 选择适配器
            adapter = self._select_adapter(tts_type)

            audio_data = await adapter.generate_voice(text)

            output_file = str(file_name)
            with open(output_file, "wb") as f:
                f.write(audio_data)

            logger.debug(f"语音生成成功: {os.path.basename(output_file)}")
            return output_file

        except Exception as e:
            logger.error(f"语音生成失败: {str(e)} 文本: \"{text}\"")
            logger.error(f"TTS服务不可达，已禁用语音，重新启动程序以刷新启动服务")
            self.enable = False
            return None
    
    async def generate_voice_stream(self, text: str, file_name: str, 
                             tts_type: str = "", lang: str ="ja") -> str | None:
        """
        生成语音文件流式

        :param text: 要转换为语音的文本
        :param file_name: 输出文件名
        :param tts_type: TTS类型，默认为空字符串表示自动选择
        :param lang: 语言，默认为"ja"
        :return: 成功时返回输出文件路径，失败时返回None
        """
        return None
