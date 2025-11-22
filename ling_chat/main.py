import os
import signal
import sys
import time
import shutil
from typing import Collection

from ling_chat.utils.runtime_path import user_data_path, third_party_path, static_path
from ling_chat.utils.load_env import load_env
from ling_chat.utils.easter_egg import get_random_loading_message

# 加载环境变量
if os.path.exists(".env"):
    load_env()
else:
    try:
        load_env(".env.example")
        load_env(user_data_path / ".env")  # 加载用户数据目录下的环境变量
    except Exception as e:
        print(f"警告：加载环境变量失败，将使用默认: {e}")

from ling_chat.core.logger import logger
from ling_chat.utils.cli_parser import get_parser
from ling_chat.third_party import install_third_party, run_third_party

def check_static_copy():
    """检查静态文件是否已经复制"""
    # 场景一，假如没有用户game_data目录，则复制
    if not os.path.exists(user_data_path / "game_data"):
        shutil.copytree(static_path / "game_data", user_data_path / "game_data")
        logger.info("静态文件已复制到用户目录")
    else:
        # 场景二，假如用户目录下有game_data目录，则检查子目录文件夹是否全部存在
        # 检查所有static_path/game_data下的子目录
        for sub_dir in os.listdir(static_path / "game_data"):
            if not os.path.exists(user_data_path / "game_data" / sub_dir):
                shutil.copytree(static_path / "game_data" / sub_dir, user_data_path / "game_data" / sub_dir)
                logger.info(f"检测到缺少文件，静态文件已复制到用户目录: {sub_dir}")
    

def handle_install(install_modules_list: Collection[str], use_mirror=False):
    """处理安装模块"""
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
    """运行CLI命令，不启动主程序"""
    if args.command == "install":
        handle_install(args.modules, use_mirror=args.mirror)
        logger.info("安装完成")
    else:
        logger.error(f"未知的CLI命令: {args.command}")


def run_main_program(args):
    """运行主程序"""
    from ling_chat.api.app_server import run_app_in_thread
    from ling_chat.core.webview import start_webview
    from ling_chat.utils.cli import print_logo
    from ling_chat.utils.voice_check import VoiceCheck
    from ling_chat.utils.function import Function

    # 控制程序退出
    should_exit = False

    def signal_handler(signum, frame):
        """处理中断信号"""
        nonlocal should_exit
        logger.info("接收到中断信号，正在关闭程序...")
        should_exit = True
        # 根据环境变量删除临时文件
        if os.environ.get("CLEAN_TEMP_FILES", "false").lower() == "true":
            Function().clean_temp_files()
            logger.info("已删除临时文件")
        else:
            logger.info("已根据环境变量禁用临时文件清理")

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    # 根据命令行参数和环境变量决定是否启用前端界面
    gui_enabled = (not args.nogui) and (os.getenv('OPEN_FRONTEND_APP', 'false').lower() == "true")
    if args.nogui:
        logger.info("启用无界面模式，前端界面已禁用")
    
    # handle_install(install_modules) TODO: 未来版本可能会启用自动安装功能

    # 启动加载动画
    selected_loading_message = get_random_loading_message()
    logger.start_loading_animation(message=selected_loading_message, animation_style="auto")

    # 检查文件完整性
    check_static_copy()

    # 处理运行模块
    handle_run(args.run or [])

    app_thread = run_app_in_thread()

    if os.getenv("VOICE_CHECK", "false").lower() == "true":

        VoiceCheck.main()
    else:
        logger.info("已根据环境变量禁用语音检查")
        
    # 检查环境变量决定是否启动前端界面
    if os.getenv("OPEN_FRONTEND_APP", "false").lower() == "true" or gui_enabled:  # fixme: 请使用 --run webview 启动前端界面
        logger.stop_loading_animation(success=True, final_message="应用加载成功")
        print_logo()
        logger.warning("[Deprecation]: 请使用 --run webview 启动前端界面，如果依然无法解决问题，请使用 --nogui 启用无前端窗口模式")
        try:
            start_webview()
        except KeyboardInterrupt:
            logger.info("用户关闭程序")
    else:
        logger.info("前端界面已禁用")
        logger.stop_loading_animation(success=True, final_message="应用加载成功")
        print_logo()
        try:
            # 循环等待
            while not should_exit and app_thread.is_alive():
                time.sleep(0.1)
        except KeyboardInterrupt:
            logger.info("用户关闭程序")

    logger.info("程序已退出")


def main():
    """主入口函数"""
    args = get_parser().parse_args()

    # 如果有CLI命令，只运行CLI命令
    if args.command:
        run_cli_command(args)
    else:
        # 否则运行主程序
        run_main_program(args)


if __name__ == "__main__":
    main()