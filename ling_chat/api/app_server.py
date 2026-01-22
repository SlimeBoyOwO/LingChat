import asyncio
import logging
import os
import threading
from contextlib import asynccontextmanager

import uvicorn
from fastapi import FastAPI, Request, Response

from ling_chat.api.routes_manager import RoutesManager
from ling_chat.core.logger import logger

from ling_chat.game_database.database import init_db
from ling_chat.game_database.managers.role_manager import RoleManager
from ling_chat.utils.runtime_path import user_data_path


@asynccontextmanager
async def lifespan(app: FastAPI):
    try:
        logger.info("正在初始化数据库...") # 骗你的其实不是在这里初始化的，在包被导入的时候自动初始化了
        init_db()

        logger.info("正在同步游戏角色数据...")
        RoleManager.sync_roles_from_folder(user_data_path / "game_data")

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

        # 获取项目日志系统的底层logging.Logger实例
        project_logger_instance = logger._logger  # 获取内部的logger实例

        # 配置uvicorn日志，使用项目自定义的日志处理器
        uvicorn_logger = logging.getLogger("uvicorn")
        uvicorn_access_logger = logging.getLogger("uvicorn.access")
        uvicorn_error_logger = logging.getLogger("uvicorn.error")
        uvicorn_asgi_logger = logging.getLogger("uvicorn.asgi")

        # 添加项目日志处理器到uvicorn日志系统
        for handler in project_logger_instance.handlers:
            uvicorn_logger.addHandler(handler)
            uvicorn_access_logger.addHandler(handler)
            uvicorn_error_logger.addHandler(handler)
            uvicorn_asgi_logger.addHandler(handler)

        # 设置日志级别
        uvicorn_log_level = getattr(logging, log_level.upper(), logging.INFO)
        uvicorn_logger.setLevel(uvicorn_log_level)
        uvicorn_access_logger.setLevel(uvicorn_log_level)
        uvicorn_error_logger.setLevel(uvicorn_log_level)
        uvicorn_asgi_logger.setLevel(uvicorn_log_level)

        # 确保uvicorn日志不会传播到上级记录器，避免重复日志
        uvicorn_logger.propagate = False
        uvicorn_access_logger.propagate = False
        uvicorn_error_logger.propagate = False
        uvicorn_asgi_logger.propagate = False

        config = uvicorn.Config(
            app,
            host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
            port=int(os.getenv('BACKEND_PORT', '8765')),
            log_level=log_level,
            access_log=True  # 启用访问日志
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
