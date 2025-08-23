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
                 default_speaker_id=4,
                 default_model_name="",
                 default_tts_type = "sbv2",
                 default_language = "ja"
                 ):
        """
        初始化VITS语音合成器

        :param original_api_url: 原始VITS API地址
        :param new_api_url: 新API地址
        :param default_speaker_id: 默认说话人ID(原始API)
        :param default_model_name: 默认模型名称(新API)
        :param audio_format: 音频格式
        :param default_lang: 默认语言
        """
        sva_api_url = os.environ.get("SIMPLE_VITS_API_URL", "http://127.0.0.1:23456/voice/vits")
        sbv2_api_url = os.environ.get("STYLE_BERT_VITS2_API_URL", "http://127.0.0.1:5000/voice")
        sbv2api_api_url = os.environ.get("SBV2API_API_URL", "http://localhost:3000/synthesize")
        bv2_api_url=os.environ.get("BERT_VITS2_API_URL", "http://127.0.0.1:6006/voice/bert-vits2")
        gpt_sovits_api_url = os.environ.get("GPT_SOVITS_API_URL", "http://127.0.0.1:9880/tts")
        gpt_sovits_ref_audio = os.environ.get("GPT_SOVITS_REF_AUDIO", "")
        gpt_sovits_prompt_text = os.environ.get("GPT_SOVITS_PROMPT_TEXT", "")
        gpt_sovits_prompt_lang = os.environ.get("GPT_SOVITS_PROMPT_LANG", "auto")

        self.format = os.environ.get("VOICE_FORMAT", "wav")

        self.sva_adapter = SVAAdapter(
            api_url=sva_api_url,
            speaker_id=default_speaker_id,
            audio_format=self.format,
            lang="ja"
        ) if sva_api_url else None

        self.sbv2_adapter = SBV2Adapter(
            api_url=sbv2_api_url,
            speaker_id=default_speaker_id,
            model_name=default_model_name,
            audio_format=self.format,
            lang="JP"
        ) if sbv2_api_url else None

        self.sbv2api_adapter = SBV2APIAdapter(
            api_url=sbv2api_api_url,
            model_name=default_model_name,
            audio_format=self.format
        ) if sbv2api_api_url else None
        
        self.bv2_adapter = BV2Adapter(
            api_url=bv2_api_url,
            speaker_id=default_speaker_id,
            audio_format=self.format,
            lang="zh"
        ) if bv2_api_url else None

        self.gsv_adapter = GPTSoVITSAdapter(
            api_url=gpt_sovits_api_url,
            ref_audio_path=gpt_sovits_ref_audio,
            prompt_text=gpt_sovits_prompt_text,
            prompt_lang=gpt_sovits_prompt_lang
        ) if gpt_sovits_api_url else None

        self.audio_format = self.format
        self.temp_dir = Path(os.environ.get("TEMP_VOICE_DIR", temp_path / "data/voice"))
        self.temp_dir.mkdir(parents=True, exist_ok=True)
        self.enable = True  # 初始化时启用

    def _select_adapter(self, params: dict):
        # TODO 这里记得后面校对一下
        # """根据参数自动选择适配器"""

        # # 优先检测SBV2API
        # if 'sbv2api_model_name' in params or self.sbv2api_adapter:
        #     if self.sbv2api_adapter is None:
        #         raise ValueError("SBV2API适配器未初始化，但传入了sbv2api_model_name参数")
        #     return self.sbv2api_adapter

        # # 优先检查新API特有的参数
        # if 'model_name' in params or 'model_id' in params:
        #     if self.sbv2_adapter is None:
        #         raise ValueError("新API适配器未初始化，但传入了model_name/model_id参数")
        #     return self.sbv2_adapter

        # # 其次检查原始API特有参数
        # if 'id' in params or ('speaker_id' in params and self.sva_adapter):
        #     if self.sva_adapter is None:
        #         raise ValueError("原始API适配器未初始化，但传入了id/speaker_id参数")
        #     return self.sva_adapter
        
        # # 最后检测GPT-SoVITS
        # if 'use_gpt_sovits_zero_shot' in params or self.gsv_adapter:
        #     if self.gsv_adapter is None:
        #         raise ValueError("GSV适配器未初始化，但传入了use_gpt_sovits_zero_shot参数")
        #     return self.gsv_adapter

        # # 默认返回新API适配器(如果存在)
        # if self.sbv2_adapter:
        #     return self.sbv2_adapter

        # # 最后尝试原始API适配器
        # if self.sva_adapter:
        #     return self.sva_adapter

        # raise ValueError("没有可用的API适配器")

        """根据tts_type选择适配器(如果传入),为空则自动选择"""
        if 'tts_type' in params and params["tts_type"] != "":
            logger.debug(f"根据参数选择TTS适配器: {params['tts_type']}")
            tts_type = params['tts_type']
            if tts_type == 'sva':
                if self.sva_adapter is None:
                    raise ValueError("原始API适配器未初始化，但传入了tts_type=sva参数")
                return self.sva_adapter
            elif tts_type == 'sbv2':
                if self.sbv2_adapter is None:
                    raise ValueError("新API适配器未初始化，但传入了tts_type=sbv参数")
                return self.sbv2_adapter
            elif tts_type == 'gsv':
                if self.gsv_adapter is None:
                    raise ValueError("GPT-SoVITS适配器未初始化，但传入了tts_type=gsv参数")
                return self.gsv_adapter
            elif tts_type == 'bv2':
                if self.bv2_adapter is None:
                    raise ValueError("Bert-Vits2适配器未初始化，但传入了tts_type=bv2参数")
                return self.bv2_adapter
            else:
                raise ValueError(f"未知的TTS类型: {tts_type}")
        elif self.sbv2_adapter is not None:
            logger.warning("未指定tts_type,默认使用sbv2")
            return self.sbv2_adapter
        else:
            raise ValueError("没有可用的API适配器")

    async def generate_voice(self, text, file_name, speaker_id=None, model_name=None, tts_type="", lang="ja", **params):
        """生成语音文件"""
        if not self.enable:
            logger.warning("TTS服务未启用，跳过语音生成")
            return None

        if not text or not text.strip():
            logger.debug("提供的文本为空，跳过语音生成")
            return None

        if tts_type in ("", "sbv2", "sva"):
            if speaker_id is not None:
                params["speaker_id" if model_name else "id"] = str(speaker_id)
        elif tts_type == "bv2":
            params["speaker_id"] = speaker_id

        if model_name is not None:
            params["model_name"] = model_name

        params["tts_type"] = tts_type
        params["lang"] = params.get("lang", "ja")

        try:
            # 选择适配器
            adapter = self._select_adapter(params)

            audio_data = await adapter.generate_voice(text, params)

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
