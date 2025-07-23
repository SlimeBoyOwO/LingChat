import os

from fastapi import FastAPI, Request
from api.chat_music import router as chat_music_router
from api.chat_history import router as chat_history_router
from api.chat_info import router as chat_info_router
from api.chat_character import router as chat_character_router
from api.chat_background import router as chat_background_router
from api.chat_main import websocket_endpoint
from api.chat_sound import router as chat_sound_router
from api.frontend_routes import router as frontend_router, get_static_files
from api.env_config import router as env_config_router

from core.logger import logger

class RoutesManager:
    def __init__(self, app):
        # 注册路由
        logger.info("注册API路由...")
        app.include_router(chat_history_router)
        app.include_router(chat_info_router)
        app.include_router(frontend_router)
        app.include_router(chat_music_router)
        app.include_router(env_config_router)
        app.include_router(chat_character_router)
        app.include_router(chat_background_router)
        app.include_router(chat_sound_router)

        app.websocket("/ws")(websocket_endpoint)

        # 静态文件服务
        logger.info("挂载静态文件服务...")
        frontend_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'frontend', 'public')
        app.mount("/", get_static_files(), name="static")