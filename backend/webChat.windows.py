import os
import asyncio
import json
import uvicorn
from fastapi import FastAPI, WebSocket, WebSocketDisconnect
from fastapi.staticfiles import StaticFiles
from fastapi.responses import HTMLResponse
from fastapi.middleware.cors import CORSMiddleware
from dotenv import load_dotenv
from core.ai_service import AIService
from core.frontend_manager import FrontendManager
from core.logger import Logger
from api.chat_history import router as chat_history_router

load_dotenv()

# ============= 初始化核心组件 =============
logger = Logger()
ai_service = AIService(logger)
app = FastAPI()
logo = [
    "", 
    "", 
    "█╗       ██╗ ███╗   ██╗  ██████╗      █████╗ ██╗  ██╗  █████╗  ████████╗",
    "██║      ██║ ████╗  ██║ ██╔════╝     ██╔═══╝ ██║  ██║ ██╔══██╗ ╚══██╔══╝",
    "██║      ██║ ██╔██╗ ██║ ██║  ███╗    ██║     ███████║ ███████║    ██║   ",
    "██║      ██║ ██║╚██╗██║ ██║   ██║    ██║     ██╔══██║ ██╔══██║    ██║   ",
    "███████╗ ██║ ██║ ╚████║ ╚██████╔╝     █████╗ ██║  ██║ ██║  ██║    ██║   ",
    "╚══════╝ ╚═╝ ╚═╝  ╚═══╝  ╚═════╝      ╚════╝ ╚═╝  ╚═╝ ╚═╝  ╚═╝    ╚═╝   "
    ]

# ============= 保留你的原始 WebSocket 处理逻辑 =============
async def handle_websocket_message(websocket, data):
    """完全复用你原有的消息处理逻辑"""
    if data.get('type') == 'message':
        logger.client_message(data)
        responses = await ai_service.process_message(data.get('content', ''))
        for response in responses:
            await websocket.send_json(response)

@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    try:
        while True:
            message = await websocket.receive()
            
            data = json.loads(message["bytes"].decode('utf-8'))

            if data.get('type') == 'ping':
                await websocket.send_json({"type": "pong"})
            elif data.get('type') == 'message':
                responses = await ai_service.process_message(data.get('content', ''))
                for response in responses:
                    await websocket.send_json(response)
                    
    except WebSocketDisconnect:
        print("客户端断开连接")

# ============= 新增 HTTP 路由 =============
app.include_router(chat_history_router)

# ============= 保留前端服务 =============
frontend_dir = os.path.join(os.path.dirname(__file__), '..', 'frontend', 'server')
if os.path.exists(frontend_dir):
    app.mount("/", StaticFiles(directory=frontend_dir, html=True), name="static")

# ============= 启动逻辑 =============
async def main():
    for line in logo:
        logger.log_text(line)
    logger.log_text("\n")

    # 启动前端
    frontend = FrontendManager(logger)
    if not frontend.start_frontend(
        frontend_dir=frontend_dir,
        port=os.getenv('FRONTEND_PORT', '3000')
    ):
        logger.error("前端启动失败")
        return

    # 启动 FastAPI（同时支持 HTTP 和 WebSocket）
    config = uvicorn.Config(
        app,
        host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
        port=int(os.getenv('FRONTEND_PORT', '8765')),
        log_level="info"
    )
    server = uvicorn.Server(config)
    await server.serve()

if __name__ == "__main__":
    asyncio.run(main())

