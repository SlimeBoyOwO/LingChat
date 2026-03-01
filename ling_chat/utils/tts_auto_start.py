import os
import subprocess
import sys
import threading
from pathlib import Path
from ling_chat.core.logger import logger


def start_tts_software():
    """
    根据环境变量自动启动语音合成软件
    支持相对路径（基于项目根目录）和绝对路径
    """
    tts_path_str = os.getenv("TTS_SOFTWARE_PATH", "").strip()
    
    if not tts_path_str:
        logger.warning("TTS_SOFTWARE_PATH 未配置，跳过自动启动语音合成软件")
        return
    
    tts_path = Path(tts_path_str)
    
    # 支持相对路径：如果是相对路径，基于项目根目录解析
    if not tts_path.is_absolute():
        from ling_chat.utils.runtime_path import package_root
        project_root = package_root.parent
        tts_path = (project_root / tts_path).resolve()
        logger.info(f"解析相对路径: {tts_path_str} -> {tts_path}")
    
    # 验证文件是否存在且为有效文件（is_file 已包含 exists 检查）
    if not tts_path.is_file():
        logger.error(f"语音合成软件路径不存在或不是有效文件: {tts_path}")
        return
    
    # 验证文件类型（仅支持 .bat 和 .exe）
    allowed_extensions = {'.bat', '.exe'}
    if tts_path.suffix.lower() not in allowed_extensions:
        logger.error(
            f"不支持的文件类型: {tts_path.suffix}。"
            f"仅支持 {', '.join(allowed_extensions)} 文件。"
            f"如需启动 Python 脚本，请创建 .bat 批处理文件封装。"
        )
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
    在独立线程中启动语音合成软件进程，在新终端窗口中运行
    """
    try:
        # 获取工作目录（TTS软件所在目录）
        cwd = tts_path.parent
        
        # 根据平台选择启动方式
        if sys.platform == 'win32':
            # Windows: 在新控制台窗口中启动
            process = subprocess.Popen(
                [str(tts_path)],
                cwd=cwd,
                creationflags=subprocess.CREATE_NEW_CONSOLE
            )
        elif sys.platform == 'darwin':
            # macOS: 使用 Terminal.app 在新终端窗口启动
            apple_script = f'''
            tell application "Terminal"
                do script "cd '{cwd}' && '{tts_path}'"
                activate
            end tell
            '''
            process = subprocess.Popen(
                ['osascript', '-e', apple_script],
                cwd=cwd
            )
        else:
            # Linux: 尝试使用常见的终端模拟器在新窗口启动
            terminal_commands = [
                ['x-terminal-emulator', '-e'],  # Debian/Ubuntu 默认
                ['gnome-terminal', '--'],       # GNOME
                ['konsole', '-e'],              # KDE
                ['xterm', '-e'],                # 经典终端
            ]
            
            launched = False
            for term_cmd in terminal_commands:
                try:
                    process = subprocess.Popen(
                        term_cmd + [str(tts_path)],
                        cwd=cwd,
                        stdout=subprocess.DEVNULL,
                        stderr=subprocess.DEVNULL
                    )
                    launched = True
                    break
                except FileNotFoundError:
                    continue
            
            if not launched:
                logger.error(
                    "未找到可用的终端模拟器。"
                    "请安装 gnome-terminal, konsole, xterm 或 x-terminal-emulator 之一。"
                )
                return
        
        logger.info(f"语音合成软件已启动，进程ID: {process.pid}")
        
    except Exception as e:
        logger.error(f"启动语音合成软件失败: {e}")
