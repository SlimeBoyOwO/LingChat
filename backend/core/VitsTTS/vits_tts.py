import os
from pathlib import Path
from .sva_adapter import SVAVitsAdapter
from .sbv_adapter import SBVVitsAdapter
from core.logger import logger

class VitsTTS:
    def __init__(self, 
                 default_speaker_id=4,
                 default_model_name=None):
        """
        初始化VITS语音合成器
        
        :param original_api_url: 原始VITS API地址
        :param new_api_url: 新API地址
        :param default_speaker_id: 默认说话人ID(原始API)
        :param default_model_name: 默认模型名称(新API)
        :param audio_format: 音频格式
        :param default_lang: 默认语言
        """
        sva_api_url=os.environ.get("SIMPLE_VITS_API_URL", "http://127.0.0.1:23456/voice/vits")
        sbv_api_url=os.environ.get("STYLE_VITS_API_URL", "http://127.0.0.1:5000/voice")

        self.format="wav"

        self.sva_adapter = SVAVitsAdapter(
            api_url=sva_api_url,
            speaker_id=default_speaker_id,
            audio_format=self.format,
            lang="ja"
        ) if sva_api_url else None
        
        self.sbv_adapter = SBVVitsAdapter(
            api_url=sbv_api_url,
            speaker_id=default_speaker_id,  # 可以共用
            model_name=default_model_name,
            audio_format=self.format,
            lang="JP"
        ) if sbv_api_url else None
        
        self.audio_format = self.format
        self.temp_dir = Path(os.environ.get("TEMP_VOICE_DIR", "data/voice"))
        self.temp_dir.mkdir(exist_ok=True)
        self.enable = True  # 初始化时启用

    def _select_adapter(self, params: dict):
        """根据参数自动选择适配器"""
        # 优先检查新API特有的参数
        if 'model_name' in params or 'model_id' in params:
            if self.sbv_adapter is None:
                raise ValueError("新API适配器未初始化，但传入了model_name/model_id参数")
            return self.sbv_adapter
        
        # 其次检查原始API特有参数
        if 'id' in params or ('speaker_id' in params and self.sva_adapter):
            if self.sva_adapter is None:
                raise ValueError("原始API适配器未初始化，但传入了id/speaker_id参数")
            return self.sva_adapter
        
        # 默认返回新API适配器(如果存在)
        if self.sbv_adapter:
            return self.sbv_adapter
        
        # 最后尝试原始API适配器
        if self.sva_adapter:
            return self.sva_adapter
        
        raise ValueError("没有可用的API适配器")

    async def generate_voice(self, text, file_name, speaker_id=None, model_name=None, **params):
        """生成语音文件"""
        if not self.enable:
            logger.warning("TTS服务未启用，跳过语音生成")
            return None
            
        if not text or not text.strip():
            logger.debug("提供的文本为空，跳过语音生成")
            return None

        # 设置说话人/模型参数
        if speaker_id is not None:
            params["speaker_id" if model_name else "id"] = str(speaker_id)
        if model_name is not None:
            params["model_name"] = model_name

        try:
            # 自动选择适配器
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