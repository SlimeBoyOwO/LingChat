import aiohttp
import asyncio
import os
from pathlib import Path
# from .logger import log_debug, log_info, log_warning, log_error, TermColors, initialize_logger
from py_ling_chat.core.logger import logger, TermColors

class VitsTTS:
    def __init__(self, api_url=None, speaker_id=4, audio_format="wav", lang="ja", enable=True):
        """
        初始化VITS语音合成器
        :param api_url: API端点地址
        :param speaker_id: 说话人ID (默认4)
        :param audio_format: 音频格式 (默认wav)
        :param lang: 语言代码 (默认ja-日语)
        :param logger: 日志记录器
        """
        self.api_url = api_url or os.environ.get("VITS_API_URL", "http://127.0.0.1:23456/voice/vits")
        self.speaker_id = speaker_id or int(os.environ.get("VITS_SPEAKER_ID", 4))
        self.format = audio_format
        self.lang = lang
        self.temp_dir = Path(os.environ.get("TEMP_VOICE_DIR", "frontend/public/audio"))
        self.temp_dir.mkdir(exist_ok=True)
        self.enable = enable
        
        # 检查语音服务可用性
        asyncio.run(self._check_service())

    async def _check_service(self):
        """检查语音服务是否可用"""
        error_message = None
        service_host_port = self.api_url.split('/')[2] 
        base_service_url = f"http://{service_host_port}" 

        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(self.api_url, timeout=2) as response:
                    if response.status < 500: 
                        self._log_tts_status(True, f"vits-simple-api 服务可达 ({service_host_port}, 状态: {response.status})")
                        self.enable = True
                        return True
                    else:
                        error_message = f"vits-simple-api 主端点响应错误 (状态码: {response.status})"

        except aiohttp.ClientConnectorError as e:
            error_message = f"vits-simple-api 连接失败 (地址: {self.api_url})"
        except asyncio.TimeoutError:
            error_message = f"vits-simple-api 连接超时 (地址: {self.api_url})"
        except Exception as e: 
            error_detail = str(e)
            if not error_detail or error_detail == "()":
                error_message = "vits-simple-api 遇到未知连接错误"
            else:
                error_message = f"vits-simple-api 连接时遇到错误: {error_detail}"
        
        self.enable = False
        final_detail_message = "vits-simple-api 不可用或响应异常，语音功能将被禁用"
        if error_message:
            final_detail_message += f" ({error_message})"
        self._log_tts_status(False, final_detail_message)
        return False

    def _log_tts_status(self, is_running: bool, details: str = None):
        """语音服务状态记录，兼容旧接口"""
        status = "语音服务已运行" if is_running else "语音服务未运行"
        status_color = TermColors.GREEN if is_running else TermColors.RED
        status_symbol = "√" if is_running else "×"
        
        if details:
            if is_running:
                logger.info(f"{status_color}{status_symbol}{TermColors.RESET} {status} - {details}")
            else:
                logger.warning(f"{status_color}{status_symbol}{TermColors.RESET} {status} - {details}")
        else:
            if is_running:
                logger.info(f"{status_color}{status_symbol}{TermColors.RESET} {status}")
            else:
                logger.warning(f"{status_color}{status_symbol}{TermColors.RESET} {status}")

    async def generate_voice(self, text, file_name, save_file=False):
        """
        异步生成语音文件
        :param text: 要合成的文本
        :param save_file: 是否保留生成的音频文件
        :return: 音频文件路径 (失败返回None)
        """
        if not self.enable:
            logger.warning("TTS服务未启用，跳过语音生成")
            return None
            
        if not text or not text.strip():
            logger.debug("提供的文本为空，跳过语音生成")
            return None

        params = {
            "text": text,
            "id": self.speaker_id,
            "format": self.format,
            "lang": self.lang
        }

        output_file = str(file_name) 
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(self.api_url, params=params, timeout=100) as response:
                    response.raise_for_status()  
                    with open(output_file, "wb") as f:
                        f.write(await response.read())
                    logger.debug(f"语音生成成功: {os.path.basename(output_file)} (文本: \"{text}\")")
            return output_file
        except aiohttp.ClientResponseError as e: 
            logger.error(f"语音生成HTTP请求失败 (URL: {e.request_info.url}, 状态: {e.status}, 消息: {e.message}) 文本: \"{text}\"")
        except aiohttp.ClientError as e:  
            logger.error(f"语音生成网络或客户端错误 (类型: {type(e).__name__}, 消息: {str(e)}) 文本: \"{text}\"")
        except asyncio.TimeoutError: 
            logger.error(f"语音生成请求超时 (URL: {self.api_url}) 文本: \"{text}\"")
        except Exception as e: 
            error_type = type(e).__name__
            error_str = str(e)
            error_repr = repr(e)
            log_msg = f"语音生成时发生未知错误 (类型: {error_type}"
            if error_str and error_str.strip():
                log_msg += f", 消息: {error_str}"
            if error_repr and error_repr.strip() and error_repr != error_str:
                 log_msg += f", 详细: {error_repr}"
            log_msg += f") 文本: \"{text}\""
            logger.error(log_msg)
        return None

    def play_voice(self, text, auto_cleanup=True):
        """
        生成并立即播放语音
        :param text: 要合成的文本
        :param auto_cleanup: 播放后是否自动删除文件
        :return: 是否成功
        """
        loop = asyncio.get_event_loop()
        audio_file = loop.run_until_complete(self.async_generate_voice(text, "temp"))
        if not audio_file:
            return False

        try:
            return True
        except Exception as e:
            print(f"[VitsTTS] 播放失败: {str(e)}")
        finally:
            if auto_cleanup and audio_file and os.path.exists(audio_file):
                os.remove(audio_file)
        return False

    def cleanup(self):
        """清理所有临时文件"""
        for file in self.temp_dir.glob(f"*.{self.format}"):
            try:
                file.unlink()
            except Exception as e:
                logger.warning(f"清理临时文件失败 {file.name}: {str(e)}")

    def __del__(self):
        """析构时自动清理"""
        self.cleanup()

# 使用示例
if __name__ == "__main__":
    # 初始化logger
    tts = VitsTTS(speaker_id=4)
    
    # 异步生成
    async def test_voice():
        audio = await tts.generate_voice("こんにちは", "greeting.wav")
        if audio:
            print(f"生成成功: {audio}")
    
    asyncio.run(test_voice())