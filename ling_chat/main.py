import os
import signal
import sys
import time

from ling_chat.utils.runtime_path import user_data_path
from ling_chat.utils.function import Function

if os.path.exists(".env"):
    Function.load_env()
else:
    try:
        Function.load_env(".env.example")
        Function.load_env(user_data_path / ".env", init=True)  # 加载用户数据目录下的环境变量
    except Exception as e:
        print(f"警告：加载环境变量失败，将使用默认: {e}")

from ling_chat.core.logger import logger
from ling_chat.api.app_server import run_app_in_thread
from ling_chat.core.webview import start_webview
from ling_chat.utils.cli import print_logo
from ling_chat.utils.voice_check import VoiceCheck


# 控制程序退出
should_exit = False

def signal_handler(signum, frame):
    """处理中断信号"""
    global should_exit
    logger.info("接收到中断信号，正在关闭程序...")
    should_exit = True

def main():
    global should_exit
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    print_logo()
    app_thread = run_app_in_thread()

    if os.getenv('VOICE_CHECK', 'false').lower() == "true":
        VoiceCheck.main()
    else:
        logger.info("已根据环境变量禁用语音检查")

    # 检查环境变量决定是否启动前端界面
    if os.getenv('OPEN_FRONTEND_APP', 'false').lower() == "true":
        try:
            start_webview()
        except KeyboardInterrupt:
            logger.info("用户关闭程序")
    else:
        logger.info("已根据环境变量禁用前端界面")
        try:
            # 循环等待
            while not should_exit and app_thread.is_alive():
                time.sleep(0.1)
        except KeyboardInterrupt:
            logger.info("用户关闭程序")
            
    logger.info("程序已退出")
    sys.exit(0)

if __name__ == "__main__":
    main()