import signal
import subprocess
import threading

from ling_chat.core.logger import logger
from ling_chat.utils.runtime_path import third_party_path


def run_in_process(args, cwd):
    process = subprocess.Popen(
        args=args,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        bufsize=1,  # 行缓冲
        text=True,  # 文本模式
    )
    return process

def run_in_thread(target):
    thread = threading.Thread(target=target)
    thread.join()

def run_vits():
    """
    启动VITS语音合成器
    """
    vits_path = third_party_path / "vits-simple-api/vits-simple-api-windows-cpu-v0.6.16"

    vits_process = run_in_process([vits_path / "py310/python.exe", "app.py"], vits_path)
    for line in vits_process.stdout:
        logger.info(f'[vits]: {line.strip()}')

    vits_process.send_signal(signal.SIGINT)


