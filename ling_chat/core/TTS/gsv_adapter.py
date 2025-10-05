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

    async def set_model(self, gpt_model_path: str, sovits_model_path: str) -> bool:
        """
        设置GPT和SoVITS模型
        
        :param gpt_model_path: GPT模型的路径
        :param sovits_model_path: SoVITS模型的路径
        :return: 是否设置成功
        """
        try:
            # 检查模型文件是否存在
            if not os.path.exists(gpt_model_path):
                logger.error(f"GPT模型文件不存在: {gpt_model_path}")
                raise FileNotFoundError(f"GPT模型文件不存在: {gpt_model_path}")
            
            if not os.path.exists(sovits_model_path):
                logger.error(f"SoVITS模型文件不存在: {sovits_model_path}")
                raise FileNotFoundError(f"SoVITS模型文件不存在: {sovits_model_path}")
            
            # 检查模型文件扩展名
            if not gpt_model_path.endswith(".ckpt"):
                logger.error(f"GPT模型文件扩展名必须为.ckpt: {gpt_model_path}")
                raise ValueError(f"GPT模型文件扩展名必须为.ckpt: {gpt_model_path}")
            
            if not sovits_model_path.endswith(".pth"):
                logger.error(f"SoVITS模型文件扩展名必须为.pth: {sovits_model_path}")
                raise ValueError(f"SoVITS模型文件扩展名必须为.pth: {sovits_model_path}")
            
            async with aiohttp.ClientSession() as session:
                # 设置GPT模型
                if gpt_model_path:
                    gpt_url = self.api_url + "/set_gpt_weights"
                    async with session.get(gpt_url, params={"weights_path": gpt_model_path}) as resp:
                        if resp.status != 200:
                            logger.error(f"GPT模型设置失败: {await resp.text()}")
                            return False
                        logger.debug(f"GPT模型设置成功: {gpt_model_path}")

                # 设置SoVITS模型
                if sovits_model_path:
                    sovits_url = self.api_url + "/set_sovits_weights"
                    async with session.get(sovits_url, params={"weights_path": sovits_model_path}) as resp:
                        if resp.status != 200:
                            logger.error(f"SoVITS模型设置失败: {await resp.text()}")
                            return False
                        logger.debug(f"SoVITS模型设置成功: {sovits_model_path}")
                
                return True
        except Exception as e:
            logger.error(f"模型设置过程中出现异常: {str(e)}")
            return False

    def get_params(self):
        return self.params