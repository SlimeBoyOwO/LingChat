import os
import threading

import uvicorn
import webview
from dotenv import load_dotenv
from fastapi import FastAPI, Request

from api.chat_history import router as chat_history_router
from api.chat_info import router as chat_info_router
from api.chat_main import websocket_endpoint
from api.chat_music import router as chat_music_router
from api.frontend_routes import router as frontend_router, get_static_files
from core.logger import logger
from py_ling_chat.utils.runtime_path import static_path

load_dotenv()

app = FastAPI()


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

app.websocket("/ws")(websocket_endpoint)

# 静态文件服务
frontend_dir = static_path.resolve()
app.mount("/", get_static_files(), name="static")  # 托管静态文件


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
    webview.create_window("Ling Chat", url=f"http://127.0.0.1:{os.getenv('BACKEND_PORT', '8765')}/", width=1200,
        height=800, resizable=True, fullscreen=False)
    webview.start(http_server=True)


def main():
    print_logo()
    app_thread = threading.Thread(target=run_app, daemon=True)
    app_thread.start()  # 启动 Uvicorn 服务器线程

    start_webview()

    app_server.should_exit = True  # 停止 Uvicorn 服务器
    app_thread.join()  # 等待线程结束


if __name__ == "__main__":
    main()
