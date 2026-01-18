import subprocess
import threading
from pathlib import Path

from ling_chat.core.logger import logger
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import third_party_path


class VoiceCheck:
    @staticmethod
    def prepare_vits_directory(vits_path: Path):
        archive_path = vits_path.parent
        if not vits_path.exists():
            vits_archive = archive_path / "vits-simple-api-windows-cpu-v*.7z"
            assert vits_archive.exists(), f"VITS语音合成器压缩包未找到，请手动下载到{vits_archive}。"
            Function.extract_archive(vits_archive, vits_path)

        assert vits_path.exists(), "VITS语音合成器目录未找到，请检查解压是否成功。"

        vits_model_path = vits_path / "data/models/YuzuSoft_Vits"
        if not vits_model_path.exists():
            vits_model_archive = archive_path / "YuzuSoft_Vits.zip"
            assert vits_model_archive.exists(), f"VITS模型压缩包未找到，请手动下载到{vits_model_archive}。"
            Function.extract_archive(vits_model_archive, vits_model_path)

        assert vits_model_path.exists(), "VITS模型目录未找到，请检查解压是否成功。"

    @staticmethod
    def run_vits_process(vits_ready_event: threading.Event, status_ok: list[bool]):
        try:
            vits_path = third_party_path / "vits-simple-api/vits-simple-api-windows-cpu-v0.6.16"
            VoiceCheck.prepare_vits_directory(vits_path)

            vits_process = subprocess.Popen(
                [vits_path / "py310/python.exe", "app.py"],
                cwd=vits_path,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                bufsize=1,  # 行缓冲
                text=True  # 文本模式
            )
            if vits_process.stdout:
                for line in vits_process.stdout:
                    logger.info(f'[vits]: {line.strip()}')
                    if "* Running on http:" in line:
                        status_ok[0] = True
                        vits_ready_event.set()
        except Exception as e:
            logger.error(f"启动VITS语音合成器失败: {e}")
            status_ok[0] = False
            vits_ready_event.set()

    @staticmethod
    def main():
            vits_ready_event = threading.Event()
            status_ok = [False]  # 用于检查VITS是否成功启动
            vits_thread = threading.Thread(target=VoiceCheck.run_vits_process, args=(vits_ready_event, status_ok), daemon=True)
            vits_thread.start()  # 启动 vits
            if not vits_ready_event.wait(timeout=120) and not status_ok[0]:
                logger.error("VITS语音合成器未能在30秒内启动，请检查配置或手动启动。")
            else:
                logger.info("VITS语音合成器已成功启动。")
