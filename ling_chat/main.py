import os
import threading

import uvicorn
import webview
from dotenv import load_dotenv
from fastapi import FastAPI, Request

from ling_chat.api.chat_main import websocket_endpoint
from ling_chat.api.chat_history import router as chat_history_router
from ling_chat.api.chat_info import router as chat_info_router
from ling_chat.api.chat_character import router as chat_character_router
from ling_chat.api.chat_music import router as chat_music_router
from ling_chat.api.chat_background import router as chat_background_router
from ling_chat.api.frontend_routes import router as frontend_router, get_static_files
from ling_chat.core.logger import logger, TermColors
from ling_chat.database import init_db
from ling_chat.database.character_model import CharacterModel
from ling_chat.utils.runtime_path import static_path, user_data_path

load_dotenv(".env.example")
load_dotenv()
load_dotenv(user_data_path / ".env")  # 加载用户数据目录下的环境变量

app = FastAPI()

def init_system():
    try:
        logger.info("正在初始化数据库...")
        init_db()

        logger.info("正在同步游戏角色数据...")
        charaModel = CharacterModel()
        charaModel.sync_characters_from_game_data(static_path)

        logger.stop_loading_animation(success=True, final_message="应用加载成功")

    except (ImportError, Exception) as e:
        logger.error(f"应用启动时发生严重错误: {e}", exc_info=True)
        logger.stop_loading_animation(success=False, final_message="应用加载失败，程序将退出")
        raise e


@app.middleware("http")
async def add_no_cache_headers(request: Request, call_next):
    response = await call_next(request)
    if not request.url.path.startswith("/api"):  # 排除API路由
        response.headers.update(
            {"Cache-Control": "no-cache, no-store, must-revalidate", "Pragma": "no-cache", "Expires": "0"})
    return response


# 注册路由
app.include_router(chat_history_router)
app.include_router(chat_info_router)
app.include_router(frontend_router)
app.include_router(chat_music_router)
app.include_router(chat_character_router)
app.include_router(chat_background_router)

app.websocket("/ws")(websocket_endpoint)

# 静态文件服务
frontend_dir = static_path.resolve()
app.mount("/", get_static_files(), name="static")  # 托管静态文件
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
        print("正在启动HTTP服务器...")
        config = uvicorn.Config(app, host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
                                port=int(os.getenv('BACKEND_PORT', '8765')), log_level="info")
        global app_server
        app_server = uvicorn.Server(config)
        app_server.run()
    except Exception as e:
        logger.error(f"服务器启动错误: {e}")


def start_webview():
    webview.create_window(
        "Ling Chat", url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/",
        width=1024, height=768,
        resizable=True, fullscreen=False
    )
    webview.start(http_server=True, icon=static_path / "resources/lingchat.ico")


def main():
    print_logo()
    app_thread = threading.Thread(target=run_app, daemon=True)
    app_thread.start()  # 启动 Uvicorn 服务器线程

    start_webview()

    app_server.should_exit = True  # 停止 Uvicorn 服务器
    app_thread.join()  # 等待线程结束


if __name__ == "__main__":
    main()
