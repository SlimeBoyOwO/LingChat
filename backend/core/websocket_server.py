# websocket_server.py
import websockets
import json
from typing import Callable, Optional

class WebSocketServer:
    def __init__(self, 
                 host: str = "0.0.0.0", 
                 port: int = 8765,
                 message_handler: Optional[Callable] = None):
        self.host = host
        self.port = port
        self.server = None
        self.message_handler = message_handler

    async def _handle_client(self, websocket):
        """处理客户端连接"""
        print("[WebSocket] 新的客户端连接建立")
        
        try:
            async for message in websocket:
                try:
                    data = json.loads(message)
                    if self.message_handler:
                        await self.message_handler(websocket, data)
                except json.JSONDecodeError:
                    print("[WebSocket] 收到无效的JSON数据")
                except Exception as e:
                    print(f"[WebSocket] 处理消息时出错: {e}")

        except websockets.exceptions.ConnectionClosed:
            print("[WebSocket] 客户端断开连接")

    async def start(self):
        """启动WebSocket服务器"""
        self.server = await websockets.serve(
            self._handle_client,
            self.host,
            self.port,
            ping_interval=20,
            max_size=2**25
        )
        print(f"[WebSocket] 服务已启动 ws://{self.host}:{self.port}")

    async def stop(self):
        """停止WebSocket服务器"""
        if self.server:
            self.server.close()
            await self.server.wait_closed()
            print("[WebSocket] 服务已停止")