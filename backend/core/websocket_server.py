# websocket_server.py
import websockets
import json
from typing import Callable, Optional
from .logger import log_debug, log_info, log_error, TermColors

class WebSocketServer:
    def __init__(self, 
                 host: str = "0.0.0.0", 
                 port: int = 8765,
                 message_handler: Optional[Callable] = None,
                 logger=None):
        self.host = host
        self.port = port
        self.server = None
        self.message_handler = message_handler
        self.logger = logger  # 保留参数以兼容旧代码，但不再使用

    async def _handle_client(self, websocket):
        """处理客户端连接"""
        log_debug("新的客户端连接")
        log_info("新的客户端连接建立")
        
        try:
            async for message in websocket:
                try:
                    data = json.loads(message)
                    if self.message_handler:
                        await self.message_handler(websocket, data)
                except json.JSONDecodeError:
                    log_error("收到无效的JSON数据")
                except Exception as e:
                    log_error(f"处理消息时出错: {e}")

        except websockets.exceptions.ConnectionClosed:
            log_info("客户端断开连接")

    async def start(self):
        """启动WebSocket服务器"""
        self.server = await websockets.serve(
            self._handle_client,
            self.host,
            self.port,
            ping_interval=20,
            max_size=2**25
        )
        self._log_backend_status(True, f"ws://{self.host}:{self.port}")

    async def stop(self):
        """停止WebSocket服务器"""
        if self.server:
            self.server.close()
            await self.server.wait_closed()
            self._log_backend_status(False, "服务已停止")
            
    def _log_backend_status(self, is_running: bool, details: str = None):
        """后端服务状态记录，兼容旧接口"""
        status = "后端服务已启动" if is_running else "后端服务未启动"
        status_color = TermColors.GREEN if is_running else TermColors.RED
        status_symbol = "✔" if is_running else "✖"
        
        if details:
            log_info(f"{status_color}{status_symbol}{TermColors.RESET} {status} - {details}")
        else:
            log_info(f"{status_color}{status_symbol}{TermColors.RESET} {status}")