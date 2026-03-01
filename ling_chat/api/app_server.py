import asyncio
import logging
import os
import threading
from contextlib import asynccontextmanager
from typing import Optional

import uvicorn
from fastapi import FastAPI, Request, Response
from fastapi.middleware.cors import CORSMiddleware

from ling_chat.api.routes_manager import RoutesManager
from ling_chat.core.logger import logger
from ling_chat.core.service_manager import service_manager

from ling_chat.game_database.database import init_db
from ling_chat.game_database.managers.role_manager import RoleManager
from ling_chat.utils.runtime_path import user_data_path


@asynccontextmanager
async def lifespan(app: FastAPI):
    try:
        logger.info("正在初始化数据库...")
        init_db()

        logger.info("正在同步游戏角色数据...")
        RoleManager.sync_roles_from_folder(user_data_path / "game_data")

        # 启动即初始化 AI 服务（避免必须进入“自由模式”才触发 /chat/info/init）
        if service_manager.ai_service is None:
            logger.info("正在初始化 AIService（启动即加载）...")
            service_manager.init_ai_service()

        yield
        # ---- shutdown ----
        if service_manager.ai_service is not None:
            try:
                await service_manager.ai_service.shutdown()
            except Exception as e:
                logger.error(f"AIService 关闭失败: {e}", exc_info=True)

    except (ImportError, Exception) as e:
        logger.error(f"应用启动时发生严重错误: {e}", exc_info=True)
        logger.stop_loading_animation(success=False, final_message="应用加载失败，程序将退出")
        raise e


app = FastAPI(lifespan=lifespan)

# CORS配置
# 默认配置
allow_origins = []
allow_origin_regex = None # 初始化正则变量
# 从环境变量读取允许的源，如果未设置则使用常见开发源
allowed_origins_str = os.getenv("ALLOWED_ORIGINS", "")
if allowed_origins_str:
    # 按逗号分割多个源
    allow_origins = [origin.strip() for origin in allowed_origins_str.split(",")]
else:
    allow_origin_regex = r"https?://.*"

app.add_middleware(
    CORSMiddleware,
    allow_origins=allow_origins,         # 如果有白名单则用这个
    allow_origin_regex=allow_origin_regex, # 如果是开发环境则用正则
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

RoutesManager(app)


@app.middleware("http")
async def add_no_cache_headers(request: Request, call_next) -> Response:
    response = await call_next(request)
    if not request.url.path.startswith("/api"):  # 排除API路由
        response.headers.update(
            {"Cache-Control": "no-cache, no-store, must-revalidate", "Pragma": "no-cache", "Expires": "0"})
    return response


app_server: uvicorn.Server
_app_thread: Optional[threading.Thread] = None
_app_loop: Optional[asyncio.AbstractEventLoop] = None


def _attach_project_handlers_to_uvicorn(project_logger_instance: logging.Logger, log_level: int) -> None:
    """
    将项目日志处理器挂接到 uvicorn 的几个常用 logger 上。
    避免重复挂接导致重复输出。
    """
    uvicorn_logger = logging.getLogger("uvicorn")
    uvicorn_access_logger = logging.getLogger("uvicorn.access")
    uvicorn_error_logger = logging.getLogger("uvicorn.error")
    uvicorn_asgi_logger = logging.getLogger("uvicorn.asgi")

    for handler in project_logger_instance.handlers:
        if handler not in uvicorn_logger.handlers:
            uvicorn_logger.addHandler(handler)
        if handler not in uvicorn_access_logger.handlers:
            uvicorn_access_logger.addHandler(handler)
        if handler not in uvicorn_error_logger.handlers:
            uvicorn_error_logger.addHandler(handler)
        if handler not in uvicorn_asgi_logger.handlers:
            uvicorn_asgi_logger.addHandler(handler)

    uvicorn_logger.setLevel(log_level)
    uvicorn_access_logger.setLevel(log_level)
    uvicorn_error_logger.setLevel(log_level)
    uvicorn_asgi_logger.setLevel(log_level)

    # 避免向上级传播导致重复日志
    uvicorn_logger.propagate = False
    uvicorn_access_logger.propagate = False
    uvicorn_error_logger.propagate = False
    uvicorn_asgi_logger.propagate = False


def _shutdown_event_loop(loop: asyncio.AbstractEventLoop) -> None:
    """
    关闭事件循环并尽可能及时回收线程池资源，避免退出阶段出现“空转等待线程”延迟。
    """
    try:
        pending = asyncio.all_tasks(loop)
        for task in pending:
            task.cancel()
        if pending:
            loop.run_until_complete(asyncio.gather(*pending, return_exceptions=True))

        # 关闭异步生成器
        loop.run_until_complete(loop.shutdown_asyncgens())

        # 关闭默认执行器，确保 anyio/to_thread 等线程池尽快退出
        shutdown_default_executor = getattr(loop, "shutdown_default_executor", None)
        if callable(shutdown_default_executor):
            loop.run_until_complete(shutdown_default_executor())
    finally:
        try:
            loop.close()
        except Exception:
            # 退出阶段避免再抛异常影响主流程
            pass


def run_app():
    """启动HTTP服务器"""
    global _app_loop
    try:
        logger.info("正在启动HTTP服务器...")
        log_level = os.getenv("LOG_LEVEL", "info").lower()
        access_log_enabled = os.getenv("BACKEND_ACCESS_LOG", "true").lower() == "true"

        # 获取项目日志系统的底层logging.Logger实例
        project_logger_instance = logger._logger  # 获取内部的logger实例

        uvicorn_log_level = getattr(logging, log_level.upper(), logging.INFO)
        _attach_project_handlers_to_uvicorn(project_logger_instance, uvicorn_log_level)

        config = uvicorn.Config(
            app,
            host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
            port=int(os.getenv('BACKEND_PORT', '8765')),
            log_level=log_level,
            access_log=access_log_enabled,  # 是否启用访问日志（默认 true）
        )
        global app_server
        app_server = uvicorn.Server(config)

        loop = asyncio.new_event_loop()
        _app_loop = loop
        asyncio.set_event_loop(loop)
        try:
            loop.run_until_complete(app_server.serve())
        except RuntimeError as e:
            # stop_app_server 可能会在退出时强制 loop.stop() 打断 run_until_complete
            logger.warning(f"后端事件循环被提前停止: {e}")

    except Exception as e:
        logger.error(f"服务器启动错误: {e}")
    finally:
        if _app_loop is not None:
            try:
                _shutdown_event_loop(_app_loop)
            finally:
                _app_loop = None
                asyncio.set_event_loop(None)
        logger.info("服务器已停止")


def run_app_in_thread():
    """在新线程中运行FastAPI应用"""
    global _app_thread
    app_thread = threading.Thread(target=run_app, daemon=True)
    app_thread.start()
    _app_thread = app_thread
    return app_thread

def stop_app_server(join_timeout_seconds: float = 8.0) -> None:
    """
    从主线程请求停止 uvicorn，并等待其线程退出。
    - 通过 should_exit 触发 uvicorn 的优雅关闭
    - 通过 call_soon_threadsafe 触发事件循环唤醒，减少退出延迟
    """
    global app_server, _app_thread, _app_loop
    try:
        if 'app_server' in globals() and app_server:
            app_server.should_exit = True
    except Exception:
        pass

    loop = _app_loop
    if loop is not None and loop.is_running():
        try:
            loop.call_soon_threadsafe(lambda: None)
        except Exception:
            pass

    th = _app_thread
    if th is not None and th.is_alive():
        th.join(timeout=join_timeout_seconds)

    # 如果仍未退出，尝试强制打断事件循环，避免线程池空转拖延退出
    th = _app_thread
    if th is not None and th.is_alive():
        try:
            if 'app_server' in globals() and app_server:
                setattr(app_server, "force_exit", True)
        except Exception:
            pass

        loop = _app_loop
        if loop is not None and loop.is_running():
            try:
                loop.call_soon_threadsafe(loop.stop)
            except Exception:
                pass

        th.join(timeout=max(2.0, join_timeout_seconds))
