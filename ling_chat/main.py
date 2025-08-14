import os
import threading
from typing import Collection

from ling_chat.api.app_server import run_app_in_thread
from ling_chat.core.webview import start_webview
from ling_chat.utils.cli import print_logo
from ling_chat.utils.cli_parser import get_parser
from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import user_data_path, third_party_path

if os.path.exists(".env"):
    Function.load_env()
else:
    Function.load_env(".env.example")
    Function.load_env(user_data_path / ".env", init=True)  # 加载用户数据目录下的环境变量

from ling_chat.core.logger import logger
from ling_chat.utils.voice_check import VoiceCheck
from ling_chat.third_party import install_third_party


def handel_install(install_modules_list: Collection[str]):
    for module in install_modules_list:
        logger.info(f"正在安装模块: {module}")
        if module == "vits":
            vits_path = third_party_path / "vits-simple-api/vits-simple-api-windows-cpu-v0.6.16"
            install_third_party.install_vits(vits_path)
            install_third_party.install_vits_model(vits_path)
        elif module == "sbv2":
            install_third_party.install_sbv2(third_party_path / "sbv2/sbv2")
        elif module == "18emo":
            install_third_party.install_18emo(third_party_path / "emotion_model_18emo")
        else:
            logger.error(f"未知的安装模块: {module}")


def handel_run(run_modules_list: Collection[str]):
    for module in run_modules_list:
        logger.info(f"正在运行模块: {module}")
        if module == "vits":
            raise NotImplementedError("vits 模块的运行函数未实现")
        elif module == "sbv2":
            raise NotImplementedError("sbv2 模块的运行函数未实现")
        elif module == "18emo":
            raise NotImplementedError("18emo 模块的运行函数未实现")
        else:
            logger.error(f"未知的运行模块: {module}")


def main():
    print_logo()
    args = get_parser().parse_args()

    handel_install(args.install or [])
    handel_run(args.run or [])

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
