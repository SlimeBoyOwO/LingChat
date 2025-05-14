# websocket_server.py
import websockets
import json
from typing import Callable, Optional
from .logger import Logger

class WebSocketServer:
    def __init__(self, 
                 host: str = "0.0.0.0", 
                 port: int = 8765,
                 message_handler: Optional[Callable] = None,
                 logger: Optional[Logger] = None):
        self.host = host
        self.port = port
        self.server = None
        self.message_handler = message_handler
        self.logger = logger or Logger()

    async def _handle_client(self, websocket):
        """处理客户端连接"""
        self.logger.debug("新的客户端连接")
        self.logger.client_connect()
        
        try:
            async for message in websocket:
                try:
                    data = json.loads(message)
                    if self.message_handler:
                        await self.message_handler(websocket, data)
                except json.JSONDecodeError:
                    self.logger.error("收到无效的JSON数据")
                except Exception as e:
                    self.logger.error(f"处理消息时出错: {e}")

        except websockets.exceptions.ConnectionClosed:
            self.logger.client_disconnect()

    async def start(self):
        """启动WebSocket服务器"""
        self.server = await websockets.serve(
            self._handle_client,
            self.host,
            self.port,
            ping_interval=20,
            max_size=2**25
        )
        self.logger.backend_status(True, f"ws://{self.host}:{self.port}")

    async def stop(self):
        """停止WebSocket服务器"""
        if self.server:
            self.server.close()
            await self.server.wait_closed()
            self.logger.backend_status(False, "服务已停止")