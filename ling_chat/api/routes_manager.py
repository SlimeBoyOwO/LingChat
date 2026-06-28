from fastapi import APIRouter

from ling_chat.api.chat_achievement import router as chat_achievement_router
from ling_chat.api.chat_advanced import router as chat_advanced_router
from ling_chat.api.chat_adventure import router as chat_adventure_router
from ling_chat.api.chat_background import router as chat_background_router
from ling_chat.api.chat_cache import router as chat_cache_router
from ling_chat.api.chat_character import router as chat_character_router
from ling_chat.api.chat_history import router as chat_history_router
from ling_chat.api.chat_info import router as chat_info_router
from ling_chat.api.chat_music import router as chat_music_router
from ling_chat.api.chat_sandbox import router as chat_sandbox_router
from ling_chat.api.chat_scene import router as chat_scene_router
from ling_chat.api.chat_schedule import router as chat_schedule_router
from ling_chat.api.chat_script import router as chat_script_router
from ling_chat.api.chat_sound import router as chat_sound_router
from ling_chat.api.console_logs import router as console_logs_router
from ling_chat.api.env_config import router as env_config_router
from ling_chat.api.file_selector import router as file_selector_router
from ling_chat.api.frontend_routes import (
    frontend_audio_path,
    frontend_path,
    get_audio_files,
    get_frontend_audio_file,
    get_static_files,
    is_frontend_available,
)
from ling_chat.api.frontend_routes import router as frontend_router
from ling_chat.api.llm_config_api import router as llm_config_router
from ling_chat.api.llm_test_api import router as llm_test_router
from ling_chat.api.new_chat_main import websocket_endpoint
from ling_chat.api.update_api import router as update_router
from ling_chat.core.logger import logger


class RoutesManager:
    @staticmethod
    def _route_exists(app, path: str, method: str) -> bool:
        wanted_method = method.upper()
        for r in getattr(app.router, "routes", []):
            if getattr(r, "path", None) != path:
                continue
            methods = getattr(r, "methods", None)
            if not methods:
                continue
            if wanted_method in methods:
                return True
        return False

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
        app.include_router(chat_cache_router)
        app.include_router(chat_script_router)
        app.include_router(console_logs_router)
        app.include_router(update_router)
        app.include_router(chat_schedule_router)
        app.include_router(chat_advanced_router)
        app.include_router(chat_scene_router)
        app.include_router(chat_adventure_router)
        app.include_router(llm_config_router)  # 注册 LLM 配置路由
        app.include_router(llm_test_router)  # 注册 LLM 测试路由
        app.include_router(chat_sandbox_router)

        app.websocket("/ws")(websocket_endpoint)

        # 静态文件服务
        logger.info("挂载静态文件服务...")
        # 前端静态音频路由（优先匹配）
        if frontend_audio_path.exists():
            # 创建一个专用路由器处理前端静态音频
            audio_router = APIRouter()

            @audio_router.get("/audio/{filename}")
            async def serve_frontend_audio(filename: str):
                return get_frontend_audio_file(filename)

            app.include_router(audio_router)
            logger.info(f"已挂载前端静态音频路由：{frontend_audio_path}")
        app.mount(
            "/audio", get_audio_files(), name="audio"
        )  # 托管临时audio文件（TTS等）
        # 挂载前端静态文件（如果目录不存在则以纯 API 模式运行）
        static_files = get_static_files()
        if static_files is not None:
            app.mount("/", static_files, name="static")  # 托管静态文件
        else:
            logger.warning(f"前端目录不存在：{frontend_path}，将以纯 API 模式运行")

        if not self._route_exists(app, "/api/v1/chat/config/settings", "GET"):
            from ling_chat.api.env_config import parse_env_file

            async def _fallback_get_settings():
                return parse_env_file()

            app.add_api_route(
                "/api/v1/chat/config/settings",
                _fallback_get_settings,
                methods=["GET"],
                include_in_schema=False,
            )
