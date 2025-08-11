import os

from ling_chat.api.app_server import run_app_in_thread
from ling_chat.core.webview import start_webview
from ling_chat.utils.cli import print_logo
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import user_data_path

if os.path.exists(".env"):
    Function.load_env()
else:
    Function.load_env(".env.example")
    Function.load_env(user_data_path / ".env", init=True)  # 加载用户数据目录下的环境变量

from ling_chat.core.logger import logger
from ling_chat.utils.voice_check import VoiceCheck


def main():
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

    app_thread.join()  # 等待应用线程结束


if __name__ == "__main__":
    main()
