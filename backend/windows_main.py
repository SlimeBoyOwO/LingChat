import os
import asyncio
from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles
from api.chat_history import router as chat_history_router
from api.chat_main import websocket_endpoint
from api.frontend_routes import router as frontend_router
from core.server import Server

app = FastAPI()

# 注册路由
app.include_router(chat_history_router)
app.include_router(frontend_router)
app.websocket("/ws")(websocket_endpoint)

# 静态文件服务
frontend_dir = os.path.join(os.path.dirname(os.path.dirname(__file__)), 'frontend', 'public')
app.mount("/", StaticFiles(directory=frontend_dir, html=True), name="static")

if __name__ == "__main__":
    server = Server(app)
    asyncio.run(server.run())