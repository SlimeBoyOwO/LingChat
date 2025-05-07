# main.py
import os
import asyncio
import json
from dotenv import load_dotenv
from core.ai_service import AIService
from core.frontend_manager import FrontendManager
from core.websocket_server import WebSocketServer

load_dotenv()

ai_service = AIService()

async def handle_websocket_message(websocket, data):
    """处理WebSocket消息"""
    if data.get('type') == 'message':
        responses = await ai_service.process_message(data.get('content', ''))
        
        if responses:
            print(f"[处理消息] 准备发送 {len(responses)} 条回复")
            for response in responses:
                await websocket.send(json.dumps(response))
                await asyncio.sleep(0.1)

async def main():
    # 初始化前端管理器
    frontend = FrontendManager()
    frontend_dir = os.path.abspath(os.path.join(
        os.path.dirname(__file__), '..', 'frontend', 'server'
    ))
    
    # 启动前端
    if not frontend.start_frontend(
        frontend_dir=frontend_dir,
        port=os.getenv('FRONTEND_PORT', '3000')
    ):
        print("[主服务] 前端启动失败，服务终止")
        return

    # 启动WebSocket服务器
    ws_server = WebSocketServer(
        host=os.getenv('BACKEND_BIND_ADDR', '0.0.0.0'),
        port=int(os.getenv('BACKEND_PORT', '8765')),
        message_handler=handle_websocket_message
    )
    
    try:
        await ws_server.start()
        print("[主服务] 服务已就绪，等待客户端连接...")
        print("[主服务] 按 Ctrl+C 停止服务")
        await asyncio.Future()  # 永久运行
    except KeyboardInterrupt:
        print("\n[主服务] 接收到终止信号")
    finally:
        await ws_server.stop()
        print("[主服务] 服务已关闭")

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except Exception as e:
        print(f"[主服务] 服务异常终止: {e}")