from ling_chat.api.chat_achievement import router as chat_achievement_router
from ling_chat.api.chat_adventure import router as chat_adventure_router
from ling_chat.api.chat_background import router as chat_background_router
from ling_chat.api.chat_character import router as chat_character_router
from ling_chat.api.chat_history import router as chat_history_router
from ling_chat.api.chat_info import router as chat_info_router
from ling_chat.api.chat_music import router as chat_music_router
from ling_chat.api.chat_scene import router as chat_scene_router
from ling_chat.api.chat_schedule import router as chat_schedule_router
from ling_chat.api.chat_script import router as chat_script_router
from ling_chat.api.chat_sound import router as chat_sound_router
from ling_chat.api.console_logs import router as console_logs_router
from ling_chat.api.env_config import router as env_config_router
from ling_chat.api.file_selector import router as file_selector_router
from ling_chat.api.frontend_routes import (
    frontend_path,
    get_audio_files,
    get_static_files,
    is_frontend_available,
)
from ling_chat.api.frontend_routes import router as frontend_router
from ling_chat.api.new_chat_main import websocket_endpoint
from ling_chat.api.update_api import router as update_router
from ling_chat.core.logger import logger


class RoutesManager:
    def __init__(self, app):
        # 注册路由
        logger.info("注册API路由...")
        app.include_router(chat_history_router)
        app.include_router(chat_info_router)
        # 前端路由仅在前端目录存在时注册
        if is_frontend_available():
            app.include_router(frontend_router)
        app.include_router(chat_music_router)
        app.include_router(env_config_router)
        app.include_router(file_selector_router)
        app.include_router(chat_achievement_router)
        app.include_router(chat_character_router)
        app.include_router(chat_background_router)
        app.include_router(chat_sound_router)
        app.include_router(chat_script_router)
        app.include_router(console_logs_router)
        app.include_router(update_router)
        app.include_router(chat_schedule_router)
        app.include_router(chat_scene_router)
        app.include_router(chat_adventure_router)

        app.websocket("/ws")(websocket_endpoint)

        # 静态文件服务
        logger.info("挂载静态文件服务...")
        app.mount("/audio", get_audio_files(), name="audio")  # 托管audio文件
        # 挂载前端静态文件（如果目录不存在则以纯 API 模式运行）
        static_files = get_static_files()
        if static_files is not None:
            app.mount("/", static_files, name="static")  # 托管静态文件
        else:
            logger.warning(f"前端目录不存在：{frontend_path}，将以纯 API 模式运行")
