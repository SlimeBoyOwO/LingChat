import os
import asyncio
from fastapi import FastAPI, Request
from fastapi.staticfiles import StaticFiles
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

# 从__init__搬过来了，先初始化数据库，初始化人物
init_db()
charaModel = CharacterModel()
charaModel.sync_characters_from_game_data("game_data")

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
app.include_router(chat_history_router)
app.include_router(chat_info_router)
app.include_router(frontend_router)
app.include_router(chat_music_router)
app.include_router(env_config_router)
app.include_router(chat_character_router)

app.websocket("/ws")(websocket_endpoint)

# 静态文件服务
frontend_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'frontend', 'public')
app.mount("/", get_static_files(), name="static")

if __name__ == "__main__":
    server = Server(app)
    asyncio.run(server.run())