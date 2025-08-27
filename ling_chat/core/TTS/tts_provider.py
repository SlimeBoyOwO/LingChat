import os
from pathlib import Path
from ling_chat.core.TTS.sva_adapter import SVAAdapter
from ling_chat.core.TTS.sbv2_adapter import SBV2Adapter
from ling_chat.core.TTS.gsv_adapter import GPTSoVITSAdapter
from ling_chat.core.TTS.sbv2api_adapter import SBV2APIAdapter
from ling_chat.core.TTS.bv2_adapter import BV2Adapter
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
        初始化VITS语音合成器

        :param default_speaker_id: 默认说话人ID
        :param default_model_name: 默认模型名称
        :param audio_format: 音频格式
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

    def init_sva_adapter(self,speaker_id: int):
        sva_api_url = os.environ.get("SIMPLE_VITS_API_URL", "http://127.0.0.1:23456/voice/vits")
        self.sva_adapter = SVAAdapter(
            api_url = sva_api_url,
            speaker_id = speaker_id,
            audio_format = self.format,
            lang = "ja"
        )
    
    def init_sbv2_adapter(self, speaker_id: int, model_name: str, language: str="ja"):
        sbv2_api_url = os.environ.get("STYLE_BERT_VITS2_API_URL", "http://127.0.0.1:5000/voice")
        self.sbv2_adapter = SBV2Adapter(
            api_url = sbv2_api_url,
            speaker_id = speaker_id,
            model_name = model_name,
            audio_format = self.format,
            lang = language
        )

    def init_sbv2api_adapter(self, model_name: str, speaker_id: int):
        sbv2api_api_url = os.environ.get("SBV2API_API_URL", "http://localhost:3000/synthesize")
        self.sbv2api_adapter = SBV2APIAdapter(
            api_url = sbv2api_api_url,
            model_name = model_name,
            speaker_id= speaker_id,
            audio_format = self.format
        )
        
    def init_bv2_adapter(self, speaker_id: int, language: str="zh"):
        bv2_api_url = os.environ.get("BERT_VITS2_API_URL", "http://127.0.0.1:6006/voice/bert-vits2")
        self.bv2_adapter = BV2Adapter(
            api_url = bv2_api_url,
            speaker_id = speaker_id,
            audio_format = self.format,
            lang = language
        )

    def init_gsv_adapter(self, ref_audio_path: str, prompt_text: str, prompt_lang: str = "auto"):
        gpt_sovits_api_url = os.environ.get("GPT_SOVITS_API_URL", "http://127.0.0.1:9880/tts")
        self.gsv_adapter = GPTSoVITSAdapter(
            api_url = gpt_sovits_api_url,
            ref_audio_path = ref_audio_path,
            prompt_text = prompt_text,
            prompt_lang = prompt_lang
        )

    def _select_adapter(self, tts_type: str):
        """根据tts_type选择适配器(如果传入),为空则自动选择"""
        if tts_type != "":
            logger.debug(f"根据参数选择TTS适配器: {tts_type}")

            if tts_type == 'sva':
                if self.sva_adapter is None:
                    raise ValueError("Vits适配器未初始化，但传入了tts_type=sva参数")
                return self.sva_adapter
            elif tts_type == 'sbv2':
                if self.sbv2_adapter is None:
                    raise ValueError("Style-Bert-Vits2适配器未初始化，但传入了tts_type=sbv参数")
                return self.sbv2_adapter
            elif tts_type == 'gsv':
                if self.gsv_adapter is None:
                    raise ValueError("GPT-SoVITS适配器未初始化，但传入了tts_type=gsv参数")
                return self.gsv_adapter
            elif tts_type == 'bv2':
                if self.bv2_adapter is None:
                    raise ValueError("Bert-Vits2适配器未初始化，但传入了tts_type=bv2参数")
                return self.bv2_adapter
            elif tts_type == 'sbv2api':
                if self.sbv2api_adapter is None:
                    raise ValueError("sbv2-api适配器未初始化，但传入了tts_type=sbv2api参数")
                return self.sbv2api_adapter
            else:
                raise ValueError(f"未知的TTS类型: {tts_type}")
        elif self.sbv2_adapter is not None:
            logger.warning("未指定tts_type,默认使用sbv2")
            return self.sbv2_adapter
        else:
            raise ValueError("没有可用的API适配器")

    async def generate_voice(self, text: str, file_name: str, 
                             tts_type: str = "", lang: str ="ja") -> str | None:
        """生成语音文件"""
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

    def cleanup(self):
        """清理所有临时文件"""
        for file in self.temp_dir.glob(f"*.{self.format}"):
            try:
                file.unlink()
            except Exception as e:
                logger.warning(f"清理临时文件失败 {file.name}: {str(e)}")
