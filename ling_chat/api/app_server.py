import os
import signal
import threading
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, Request, Response

from ling_chat.api.routes_manager import RoutesManager
from ling_chat.core.logger import logger
from ling_chat.database import init_db
from ling_chat.database.character_model import CharacterModel
from ling_chat.utils.runtime_path import user_data_path


@asynccontextmanager
async def lifespan(app: FastAPI):
    try:
        logger.info("正在初始化数据库...")
        init_db()

        logger.info("正在同步游戏角色数据...")
        CharacterModel.sync_characters_from_game_data(user_data_path / "game_data")

        logger.stop_loading_animation(success=True, final_message="应用加载成功")
        yield

    except (ImportError, Exception) as e:
        logger.error(f"应用启动时发生严重错误: {e}", exc_info=True)
        logger.stop_loading_animation(success=False, final_message="应用加载失败，程序将退出")
        raise e


app = FastAPI(lifespan=lifespan)
RoutesManager(app)


@app.middleware("http")
async def add_no_cache_headers(request: Request, call_next) -> Response:
    response = await call_next(request)
    if not request.url.path.startswith("/api"):  # 排除API路由
        response.headers.update(
            {"Cache-Control": "no-cache, no-store, must-revalidate", "Pragma": "no-cache", "Expires": "0"})
    return response


app_server: uvicorn.Server


def run_app():
    try:
        logger.info("正在启动HTTP服务器...")
        config = uvicorn.Config(app, host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
                                port=int(os.getenv('BACKEND_PORT', '8765')),
                                log_level=os.getenv("LOG_LEVE", "info").lower())
        global app_server
        app_server = uvicorn.Server(config)

        app_server.run()
    except Exception as e:
        logger.error(f"服务器启动错误: {e}")


def signal_handler(signum, frame):
    """处理中断"""
    logger.info("接收到中断信号，正在关闭程序...")
    app_server.should_exit = True
    app_server.shutdown()


def run_app_in_thread():
    """在新线程中运行FastAPI应用"""
    app_thread = threading.Thread(target=run_app, daemon=True)
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    app_thread.start()
    return app_thread
