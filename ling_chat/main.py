import os
import threading
import signal
import sys

import uvicorn
import webview
from fastapi import FastAPI, Request

from ling_chat.utils.function import Function
from ling_chat.utils.runtime_path import static_path, user_data_path

if os.path.exists(".env"):
    Function.load_env()
else:
    Function.load_env(".env.example")
    Function.load_env(user_data_path / ".env" , init=True) # 加载用户数据目录下的环境变量

from ling_chat.api.routes_manager import RoutesManager
from ling_chat.core.logger import logger, TermColors
from ling_chat.database import init_db
from ling_chat.database.character_model import CharacterModel
from ling_chat.utils.voice_check import VoiceCheck

app = FastAPI()

def init_system():
    try:
        logger.info("正在初始化数据库...")
        init_db()

        logger.info("正在同步游戏角色数据...")
        CharacterModel.sync_characters_from_game_data(user_data_path / "game_data")

        logger.stop_loading_animation(success=True, final_message="应用加载成功")

    except (ImportError, Exception) as e:
        logger.error(f"应用启动时发生严重错误: {e}", exc_info=True)
        logger.stop_loading_animation(success=False, final_message="应用加载失败，程序将退出")
        raise e

init_system()  # 初始化系统

@app.middleware("http")
async def add_no_cache_headers(request: Request, call_next):
    response = await call_next(request)
    if not request.url.path.startswith("/api"):  # 排除API路由
        response.headers.update(
            {"Cache-Control": "no-cache, no-store, must-revalidate", "Pragma": "no-cache", "Expires": "0"})
    return response


routes_manager = RoutesManager(app)    # 挂载路由
logger.info_color("所有组件初始化完毕，服务器准备就绪。", color=TermColors.CYAN)


def print_logo():
    logo = [  #
        "█╗       ██╗ ███╗   ██╗  ██████╗      █████╗ ██╗  ██╗  █████╗  ████████╗",
        "██║      ██║ ████╗  ██║ ██╔════╝     ██╔═══╝ ██║  ██║ ██╔══██╗ ╚══██╔══╝",
        "██║      ██║ ██╔██╗ ██║ ██║  ███╗    ██║     ███████║ ███████║    ██║   ",
        "██║      ██║ ██║╚██╗██║ ██║   ██║    ██║     ██╔══██║ ██╔══██║    ██║   ",
        "███████╗ ██║ ██║ ╚████║ ╚██████╔╝     █████╗ ██║  ██║ ██║  ██║    ██║   ",
        "╚══════╝ ╚═╝ ╚═╝  ╚═══╝  ╚═════╝      ╚════╝ ╚═╝  ╚═╝ ╚═╝  ╚═╝    ╚═╝   ",  #
    ]
    for line in logo:
        print(line)


app_server: uvicorn.Server

def run_app():
    try:
        logger.info("正在启动HTTP服务器...")
        config = uvicorn.Config(app, host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
                                port=int(os.getenv('BACKEND_PORT', '8765')), log_level=os.getenv("LOG_LEVE","info").lower())
        global app_server
        app_server = uvicorn.Server(config)
        app_server.run()
    except Exception as e:
        logger.error(f"服务器启动错误: {e}")

def start_webview():
    try:
        webview.create_window(
            "Ling Chat", url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/",
            width=1024, height=600,
            resizable=True, fullscreen=False
        )
        webview.start(
            http_server=True,
            icon=str(static_path / "game_data/resources/lingchat.ico"),
            storage_path=str(user_data_path / "webview_storage_path"),
        )
    except KeyboardInterrupt:
        logger.info("WebView被中断")

should_exit = False
def signal_handler(signum, frame):
    """处理中断"""
    global should_exit
    logger.info("接收到中断信号，正在关闭程序...")
    should_exit = True
    if 'app_server' in globals():
        app_server.should_exit = True
    sys.exit(0)

def main():
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    print_logo()

    app_thread = threading.Thread(target=run_app, daemon=True)
    app_thread.start()  # 启动 Uvicorn 服务器线程

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
        # 让主线程等待并保持后端服务运行
        try:
            while not should_exit:
                app_thread.join(timeout=1)
        except KeyboardInterrupt:
            logger.info("正在关闭服务...")
            signal_handler(signal.SIGINT, None)

    # 确保服务器关闭
    if 'app_server' in globals():
        app_server.should_exit = True
    app_thread.join(timeout=5)


if __name__ == "__main__":
    main()