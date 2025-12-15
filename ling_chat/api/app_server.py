import os
import threading
import asyncio
import logging
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
    """启动HTTP服务器"""
    try:
        logger.info("正在启动HTTP服务器...")
        log_level = os.getenv("LOG_LEVEL", "info").lower()
        
        # 更彻底地禁用 uvicorn 的默认日志配置
        logging.getLogger("uvicorn").handlers.clear()
        logging.getLogger("uvicorn.access").handlers.clear()
        logging.getLogger("uvicorn.error").handlers.clear()
        logging.getLogger("uvicorn.asgi").handlers.clear()
        
        # 禁用 uvicorn 的 propagate，防止日志传播到根日志记录器
        logging.getLogger("uvicorn").propagate = False
        logging.getLogger("uvicorn.access").propagate = False
        logging.getLogger("uvicorn.error").propagate = False
        logging.getLogger("uvicorn.asgi").propagate = False
        
        config = uvicorn.Config(
            app, 
            host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
            port=int(os.getenv('BACKEND_PORT', '8765')),
            log_level=log_level,
            log_config=None,  # 不使用 uvicorn 默认的日志配置
            access_log=False  # 禁用访问日志
        )
        global app_server
        app_server = uvicorn.Server(config)

        asyncio.set_event_loop(asyncio.new_event_loop())
        loop = asyncio.get_event_loop()
        loop.run_until_complete(app_server.serve())

    except Exception as e:
        logger.error(f"服务器启动错误: {e}")
    finally:
        logger.info("服务器已停止")


def run_app_in_thread():
    """在新线程中运行FastAPI应用"""
    app_thread = threading.Thread(target=run_app, daemon=True)
    app_thread.start()
    return app_thread

async def shutdown_server():
    """用于关闭服务器"""
    global app_server
    if 'app_server' in globals() and app_server:
        app_server.should_exit = True