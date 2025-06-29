# 这个模块在正式导入之前，用于添加小动画以点缀漫长的等待

import os
import sys

try:
    from core.logger import logger, TermColors
    from utils.easter_egg import get_random_loading_message # <--- 新增导入
except ImportError as e:
    print(f"错误：无法导入核心模块。请确保 'core' 和 'utils' 目录位置正确。 {e}")
    sys.exit(1)

selected_loading_message = get_random_loading_message()
logger.start_loading_animation(message=selected_loading_message, animation_style="clock")
load_success = False
try:

###################你应该把正式的导入写在这里###################
    import asyncio
    from fastapi import FastAPI, Request
    from api.chat_music import router as chat_music_router
    from api.chat_history import router as chat_history_router
    from api.chat_info import router as chat_info_router
    from api.chat_character import router as chat_character_router
    from api.chat_main import websocket_endpoint
    from api.frontend_routes import router as frontend_router, get_static_files
    from api.env_config import router as env_config_router
    from core.server import Server
    from database.database import init_db
    from database.character_model import CharacterModel
###################你应该把正式的导入写在这里###################

    logger.info("正在初始化数据库...")
    init_db()
    
    logger.info("正在同步游戏角色数据...")
    charaModel = CharacterModel()
    charaModel.sync_characters_from_game_data("game_data")

    load_success = True

except (ImportError, Exception) as e:
    logger.error(f"应用启动时发生严重错误: {e}", exc_info=True)
    load_success = False

finally:
    if load_success:
        logger.stop_loading_animation(success=True, final_message="应用加载成功")
    else:
        logger.stop_loading_animation(success=False, final_message="应用加载失败，程序将退出")
        sys.exit(1)

###################下面进入主程序###################

app = FastAPI()

@app.middleware("http")
async def add_no_cache_headers(request: Request, call_next):
    response = await call_next(request)
    if not request.url.path.startswith("/api"):  # 排除API路由
        response.headers.update({
            "Cache-Control": "no-cache, no-store, must-revalidate",
            "Pragma": "no-cache",
            "Expires": "0"
        })
    return response

# 注册路由
logger.info("注册API路由...")
app.include_router(chat_history_router)
app.include_router(chat_info_router)
app.include_router(frontend_router)
app.include_router(chat_music_router)
app.include_router(env_config_router)
app.include_router(chat_character_router)

app.websocket("/ws")(websocket_endpoint)

# 静态文件服务
logger.info("挂载静态文件服务...")
frontend_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'frontend', 'public')
app.mount("/", get_static_files(), name="static")

logger.info_color("所有组件初始化完毕，服务器准备就绪。", color=TermColors.CYAN)

if __name__ == "__main__":
    server = Server(app)
    asyncio.run(server.run())