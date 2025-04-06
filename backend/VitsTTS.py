import aiohttp
import asyncio
from playsound import playsound
import os
from pathlib import Path

class VitsTTS:
    def __init__(self, api_url="http://192.168.31.228:23456/voice/vits", speaker_id=4, audio_format="wav", lang="ja"):
        """
        初始化VITS语音合成器
        :param api_url: API端点地址
        :param speaker_id: 说话人ID (默认4)
        :param audio_format: 音频格式 (默认wav)
        :param lang: 语言代码 (默认ja-日语)
        """
        self.api_url = api_url
        self.speaker_id = speaker_id
        self.format = audio_format
        self.lang = lang
        self.temp_dir = Path("temp_voice")
        self.temp_dir.mkdir(exist_ok=True)

    async def generate_voice(self, text, file_name, save_file=False):
        """
        异步生成语音文件
        :param text: 要合成的文本
        :param save_file: 是否保留生成的音频文件
        :return: 音频文件路径 (失败返回None)
        """
        params = {
            "text": text,
            "id": self.speaker_id,
            "format": self.format,
            "lang": self.lang
        }

        output_file = f"{file_name}"
        try:
            async with aiohttp.ClientSession() as session:
                async with session.get(self.api_url, params=params, timeout=10) as response:
                    response.raise_for_status()
                    with open(output_file, "wb") as f:
                        f.write(await response.read())
            return str(output_file)
        except aiohttp.ClientError as e:
            print(f"[VitsTTS] 请求失败: {str(e)}")
        except Exception as e:
            print(f"[VitsTTS] 生成失败: {str(e)}")
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
            playsound(audio_file)
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
                print(f"[VitsTTS] 清理失败 {file.name}: {str(e)}")

    def __del__(self):
        """析构时自动清理"""
        self.cleanup()

# 使用示例
if __name__ == "__main__":
    tts = VitsTTS(speaker_id=4)
    
    # 方式1: 仅生成不播放 (异步)
    loop = asyncio.get_event_loop()
    audio = loop.run_until_complete(tts.async_generate_voice("こんにちは", "greeting"))
    if audio:
        playsound(audio)
        os.remove(audio)  # 手动清理
    
    # 方式2: 生成并立即播放(自动清理)
    tts.play_voice("ありがとうございます")
    
    # 长时间运行时的定期清理
    tts.cleanup()