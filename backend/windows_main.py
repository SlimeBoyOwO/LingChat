import os
import asyncio
from fastapi import FastAPI, Request
from fastapi.staticfiles import StaticFiles
from api.chat_history import router as chat_history_router
from api.chat_info import router as chat_info_router
from api.chat_main import websocket_endpoint
from api.frontend_routes import router as frontend_router, get_static_files
from core.server import Server

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

app.websocket("/ws")(websocket_endpoint)

# 静态文件服务
frontend_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'frontend', 'public')
app.mount("/", get_static_files(), name="static")  # 托管静态文件

if __name__ == "__main__":
    server = Server(app)
    asyncio.run(server.run())