import os
import shutil
import signal
import time
import threading
import sys
from typing import Collection

from ling_chat.core.logger import logger
from ling_chat.third_party import install_third_party, run_third_party
from ling_chat.utils.cli_parser import get_parser
from ling_chat.utils.easter_egg import get_random_loading_message
from ling_chat.utils.runtime_path import static_path, third_party_path, user_data_path
from ling_chat.core.webview import start_webview

exit_event = threading.Event()


def _module_signal_handler(signum, frame):
    exit_event.set()

def check_static_copy():
    if not os.path.exists(user_data_path / "game_data"):
        shutil.copytree(static_path / "game_data", user_data_path / "game_data")
        logger.info("静态文件已复制到用户目录")
    else:
        for sub_dir in os.listdir(static_path / "game_data"):
            if not os.path.exists(user_data_path / "game_data" / sub_dir):
                shutil.copytree(static_path / "game_data" / sub_dir, user_data_path / "game_data" / sub_dir)
                logger.info(f"检测到缺少文件，静态文件已复制到用户目录: {sub_dir}")


def handle_install(install_modules_list: Collection[str], use_mirror=False):
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
        elif module == "rag":
            install_third_party.install_rag_model(use_mirror=use_mirror)
        else:
            logger.error(f"未知的安装模块: {module}")


def handle_run(run_modules_list: Collection[str]):
    """处理运行模块"""
    for module in run_modules_list:
        logger.info(f"正在运行模块: {module}")
        if module == "vits":
            run_third_party.run_in_thread(run_third_party.run_vits)
        elif module == "sbv2":
            raise NotImplementedError("sbv2 模块的运行函数未实现")
        elif module == "18emo":
            raise NotImplementedError("18emo 模块的运行函数未实现")
        elif module == "webview":
            run_third_party.run_webview()
        else:
            logger.error(f"未知的运行模块: {module}")


def run_cli_command(args):
    if args.command == "install":
        handle_install(args.modules, use_mirror=args.mirror)
        logger.info("安装完成")
    else:
        logger.error(f"未知的CLI命令: {args.command}")


def run_main_program(args,is_wv=False):
    from ling_chat.api.app_server import run_app_in_thread, stop_app_server

    from ling_chat.utils.cli import print_logo
    from ling_chat.utils.function import Function
    from ling_chat.utils.voice_check import VoiceCheck
    try:
        if threading.current_thread() is threading.main_thread():
            def _local_signal_handler(signum, frame):
                logger.info("接收到中断信号，正在关闭程序...")
                exit_event.set()

        try:
            from ling_chat.core.achievement_manager import AchievementManager
            AchievementManager.get_instance().save_if_dirty()
        except Exception as e:
            logger.error(f"保存成就数据时出错: {e}")

        signal.signal(signal.SIGINT,_local_signal_handler)
        signal.signal(signal.SIGTERM, _local_signal_handler)
    except Exception:
        logger.debug("无法在当前环境注册局部信号处理器")
    if args.nogui:
        logger.info("启用无界面模式，前端界面已禁用")

    # handle_install(install_modules) TODO: 未来版本可能会启用自动安装功能
    selected_loading_message = get_random_loading_message()
    logger.start_loading_animation(message=selected_loading_message, animation_style="auto")
    app_thread: threading.Thread | None = None
    try:
        check_static_copy()
        handle_run(args.run or [])
        app_thread = run_app_in_thread()
        if os.getenv("VOICE_CHECK", "false").lower() == "true":
            VoiceCheck.main()
        else:
            logger.info("已根据环境变量禁用语音检查")

        # 检查环境变量决定是否启动前端界面
        if (os.getenv("OPEN_FRONTEND_APP", "false").lower() == "true" and not args.nogui) or args.gui:
            logger.stop_loading_animation(success=True, final_message="应用加载成功")
            print_logo()
            logger.warning("[前端界面已启用，可使用 --nogui 启用无前端窗口模式")
            try:
                start_webview()
            except KeyboardInterrupt:
                logger.info("用户关闭程序")
        else:
            logger.info("前端界面已禁用，可使用请使用 --gui 强制启用前端窗口")
            logger.stop_loading_animation(success=True, final_message="应用加载成功")
            print_logo()
            try:
                while (not exit_event.is_set()) and (app_thread is not None) and app_thread.is_alive():
                    time.sleep(0.1)
            except KeyboardInterrupt:
                logger.info("用户关闭程序")
    finally:
        # 主动停止后端，避免 anyio/线程池空转导致退出延迟
        try:
            stop_app_server()
        except Exception as e:
            logger.error(f"停止后端服务失败: {e}", exc_info=True)
        logger.info("程序已退出")
        try:
            logger.shutdown()
        except Exception:
            pass


def main():
    args = get_parser().parse_args()
    if args.command:
         run_cli_command(args)
    else:
         run_main_program(args,False)

if __name__ == "__main__":
    main()
