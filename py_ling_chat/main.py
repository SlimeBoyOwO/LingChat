import asyncio
import os
from pathlib import Path

import uvicorn
from api.chat_history import router as chat_history_router
from api.chat_info import router as chat_info_router
from api.chat_main import websocket_endpoint
from api.chat_music import router as chat_music_router
from api.frontend_routes import router as frontend_router, get_static_files
from dotenv import load_dotenv
from fastapi import FastAPI, Request

from core.logger import logger

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
frontend_dir = Path(__file__).resolve() / 'static'
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


async def run(app: FastAPI):
    config = uvicorn.Config(app, host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
                            port=int(os.getenv('BACKEND_PORT', '8765')), log_level="info")

    server = uvicorn.Server(config)

    try:
        print("正在启动HTTP服务器...")
        server_task = asyncio.create_task(server.serve())

        while not server.started:
            await asyncio.sleep(0.1)

        await server_task

    except Exception as e:
        logger.error(f"服务器启动错误: {e}")


def start_webview():
    print('TODO: 启动WebView')


def main():
    print_logo()
    asyncio.run(run(app))


if __name__ == "__main__":
    main()
