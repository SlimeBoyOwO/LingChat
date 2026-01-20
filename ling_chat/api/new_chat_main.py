import asyncio
import json
import traceback
import uuid

from fastapi import WebSocket, WebSocketDisconnect

from ling_chat.core.logger import logger
from ling_chat.core.messaging.broker import message_broker
from ling_chat.core.service_manager import service_manager


class WebSocketManager:
    def __init__(self):
        self.active_connections: dict[str, WebSocket] = {}

    async def handle_websocket(self, websocket: WebSocket, client_id: str):
        """处理WebSocket连接的主方法"""
        # 这里不再调用 websocket.accept()，因为已经在端点中调用
        self.active_connections[client_id] = websocket

        # 创建发送任务
        send_task = asyncio.create_task(
            self._send_messages(websocket, client_id)
        )

        try:
            # 处理接收的消息
            async for message in self._receive_messages(websocket):
                await self._handle_client_message(websocket, client_id, message)

        except WebSocketDisconnect:
            logger.info(f"客户端 {client_id} 正常断开连接")
        except Exception as e:
            logger.error(f"WebSocket连接错误: {e}")
            traceback.print_exc()
        finally:
            # 清理资源
            self.active_connections.pop(client_id, None)
            send_task.cancel()
            try:
                await send_task
            except asyncio.CancelledError:
                pass
            if service_manager.ai_service is not None:
                await service_manager.ai_service.remove_client(client_id)
            logger.info(f"客户端 {client_id} 连接已清理")

    async def _receive_messages(self, websocket: WebSocket):
        """接收消息的通用逻辑"""
        while True:
            try:
                data = await websocket.receive()

                # 处理断开连接
                if data.get("type") == "websocket.disconnect":
                    logger.info("收到断开连接消息")
                    break

                # 处理文本消息
                if "text" in data:
                    yield json.loads(data["text"])
                # 处理二进制消息（如果需要）
                elif "bytes" in data:
                    # 可以处理二进制消息
                    pass

            except (WebSocketDisconnect, ConnectionResetError, RuntimeError):
                break
            except json.JSONDecodeError:
                logger.error("消息JSON格式错误")
                await websocket.send_json({
                    "type": "error",
                    "message": "消息格式错误"
                })

    async def _handle_client_message(self, websocket: WebSocket, client_id: str, message: dict):
        """处理客户端消息"""
        message_type = message.get('type')

        if message_type == 'ping':
            await websocket.send_json({"type": "pong"})
        elif message_type == 'message':
            await self._handle_user_message(client_id, message)
        else:
            logger.warning(f"未知消息类型: {message_type}")

    async def _handle_user_message(self, client_id: str, message: dict):
        """处理用户发送的消息"""
        logger.info(f"来自客户端 {client_id} 的消息: {message}")

        ai_service = service_manager.ai_service
        if ai_service is None:
            logger.error("AI服务未初始化")
            # 可以发送错误消息给前端
            await self.send_to_client(client_id, {
                "type": "error",
                "message": "服务未就绪，请刷新页面"
            })
            return

        user_message = message.get('content', '')

        if user_message == "/开始剧本":
            asyncio.create_task(ai_service.start_script())
            logger.info("开始进行剧本模式")
        elif user_message == "/查看记忆":
            ai_service.show_memory()
        else:
            if ai_service.scripts_manager.is_running:
                asyncio.create_task(
                    message_broker.enqueue_ai_script_message(client_id, user_message)
                )
            else:
                asyncio.create_task(
                    message_broker.enqueue_ai_message(client_id, user_message)
                )


    async def _send_messages(self, websocket: WebSocket, client_id: str):
        """从消息队列中获取并发送消息"""
        try:
            async for message in message_broker.subscribe(client_id):
                if message:
                    logger.info(f"向客户端 {client_id} 发送消息: {message}")
                    try:
                        await websocket.send_json(message)
                    except (WebSocketDisconnect, RuntimeError):
                        break
                    except Exception as e:
                        logger.error(f"发送消息失败: {e}")
                        break
        except Exception as e:
            logger.error(f"消息订阅异常: {e}")

    async def send_to_client(self, client_id: str, message: dict):
        """直接向指定客户端发送消息"""
        if client_id in self.active_connections:
            try:
                await self.active_connections[client_id].send_json(message)
            except Exception as e:
                logger.error(f"直接发送消息失败: {e}")

# 改进的端点函数
async def websocket_endpoint(websocket: WebSocket):
    """WebSocket端点 - 改进版本"""
    # 首先接受连接
    await websocket.accept()

    # 后端分配client_id
    client_id = f"client_{uuid.uuid4().hex}"

    # 立即通知客户端分配的ID
    await websocket.send_json({
        "type": "connection_established",
        "client_id": client_id
    })

    try:
        await ws_manager.handle_websocket(websocket, client_id)
    except Exception as e:
        logger.error(f"WebSocket端点异常: {e}")
        try:
            await websocket.close(code=1011, reason="服务器错误")
        except:
            pass

ws_manager = WebSocketManager()
