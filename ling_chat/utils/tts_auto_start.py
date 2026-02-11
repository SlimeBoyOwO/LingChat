import os
import subprocess
import threading
from pathlib import Path
from ling_chat.core.logger import logger


def start_tts_software():
    """
    根据环境变量自动启动语音合成软件
    """
    tts_path_str = os.getenv("TTS_SOFTWARE_PATH", "").strip()
    
    if not tts_path_str:
        logger.warning("TTS_SOFTWARE_PATH 未配置，跳过自动启动语音合成软件")
        return
    
    tts_path = Path(tts_path_str)
    
    if not tts_path.exists():
        logger.error(f"语音合成软件路径不存在: {tts_path}")
        return
    
    if not tts_path.is_file():
        logger.error(f"语音合成软件路径不是有效的文件: {tts_path}")
        return
    
    # 在后台线程中启动
    thread = threading.Thread(
        target=_run_tts_process,
        args=(tts_path,),
        daemon=True,
        name="TTS-Auto-Start"
    )
    thread.start()
    logger.info(f"正在启动语音合成软件: {tts_path}")


def _run_tts_process(tts_path: Path):
    """
    在独立线程中启动语音合成软件进程
    """
    try:
        # 获取工作目录（TTS软件所在目录）
        cwd = tts_path.parent
        
        # 根据文件扩展名选择启动方式
        if tts_path.suffix.lower() == '.py':
            # Python 脚本
            process = subprocess.Popen(
                ["python", str(tts_path)],
                cwd=cwd,
                creationflags=subprocess.CREATE_NEW_CONSOLE
            )
        else:
            # 可执行文件 (.exe, .bat 等)
            process = subprocess.Popen(
                [str(tts_path)],
                cwd=cwd,
                creationflags=subprocess.CREATE_NEW_CONSOLE
            )
        
        logger.info(f"语音合成软件已启动，进程ID: {process.pid}")
        
    except Exception as e:
        logger.error(f"启动语音合成软件失败: {e}")
